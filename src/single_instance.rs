use crate::single_instance::NextStep::{Abort, Continue};
use anyhow::Error;
use futures::channel::mpsc;
use futures::channel::mpsc::UnboundedReceiver;
use log::error;
use serde_json::json;
use std::env;
use std::os::unix::net::UnixDatagram;
use std::thread::spawn;

const SOCKET_PATH: &'static str = "/tmp/share-rs.socket";

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
    // remove the socket if the process listening on it has died
    if let Err(e) = UnixDatagram::unbound()?.connect(SOCKET_PATH) {
        if e.kind() == std::io::ErrorKind::ConnectionRefused {
            std::fs::remove_file(SOCKET_PATH)?;
        }
    }

    match UnixDatagram::bind(SOCKET_PATH) {
        Ok(socket) => {
            let (tx, rx) = mpsc::unbounded();
            spawn(move || {
                let mut buf = vec![0; 1024];
                loop {
                    match socket.recv(buf.as_mut_slice()) {
                        Ok(len) => {
                            let json_str = String::from_utf8_lossy(&buf[..len]).to_string();
                            let parsed: serde_json::error::Result<Vec<String>> =
                                serde_json::from_str(&json_str);
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
                                    error!("Failed to parse {} to Vec<String>, {}", json_str, e);
                                }
                            }
                        }
                        Err(e) => {
                            error!(
                                "Failed to recv from socket {}, {}",
                                SOCKET_PATH,
                                e.to_string()
                            );
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
                UnixDatagram::unbound()?.send_to(args.as_bytes(), SOCKET_PATH)?;
                Ok(Abort)
            } else {
                Err(Error::from(e))
            }
        }
    }
}
