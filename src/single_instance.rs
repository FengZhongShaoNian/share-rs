use crate::single_instance::NextStep::{Abort, Continue};
use anyhow::Error;
use futures::channel::mpsc;
use futures::channel::mpsc::UnboundedReceiver;
use log::{error, info};
use serde_json::json;
use std::env;
use std::io::{Read, Write};
use std::net::Shutdown;
use std::thread::spawn;

#[cfg(windows)]
use uds_windows::{UnixStream, UnixListener};

#[cfg(unix)]
use std::os::unix::net::{UnixStream, UnixListener};

#[derive(Debug)]
pub enum NextStep {
    Continue(UnboundedReceiver<OpenRequest>),
    Abort,
}

#[derive(Default)]
pub struct OpenRequest {
    pub args: Vec<String>,
}

// 如果已经有进行存在，那么将启动参数发送给已存在的进程处理
// 如果没有已存在的进程，那么监听指定的socket，当有人
pub fn check_single_instance() -> anyhow::Result<NextStep> {
    let socket_path = dirs::cache_dir().ok_or(anyhow::anyhow!("no cache dir"))?;
    let socket_path =  socket_path.join("share-rs.socket");
    info!("checking single instance, socket path: {}", socket_path.display());

    // remove the socket if the process listening on it has died
    if let Err(e) = UnixStream::connect(&socket_path) {
        if e.kind() == std::io::ErrorKind::ConnectionRefused {
            std::fs::remove_file(&socket_path)?;
        }
    }

    match UnixListener::bind(&socket_path) {
        Ok(listener) => {
            let (tx, rx) = mpsc::unbounded();
            spawn(move || {
                for stream in listener.incoming() {
                    match stream {
                        Ok(mut stream) => {
                            let mut response = String::new();
                            stream.read_to_string(&mut response).unwrap();

                            let parsed: serde_json::error::Result<Vec<String>> =
                                serde_json::from_str(&response);
                            match parsed {
                                Ok(args) => {
                                    let result = tx.unbounded_send(OpenRequest { args });
                                    if let Err(e) = result {
                                        error!(
                                            "Failed to send args by UnboundedSender<OpenRequest>, {e}"
                                        );
                                    }
                                }
                                Err(e) => {
                                    error!("Failed to parse {} to Vec<String>, {}", response, e);
                                }
                            }
                        }
                        Err(err) => {
                            error!("Failed to accept client: {}", err);
                        }
                    }
                }
            });
            Ok(Continue(rx))
        }
        Err(e) => {
            if e.kind() == std::io::ErrorKind::AddrInUse {
                let args: Vec<String> = env::args().collect();
                let args = json!(args);
                let args = serde_json::to_string(&args)?;
                let mut stream = UnixStream::connect(&socket_path)?;
                stream.write_all(args.as_bytes())?;
                stream.shutdown(Shutdown::Both)?;
                Ok(Abort)
            } else {
                Err(Error::from(e))
            }
        }
    }
}
