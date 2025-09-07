use crate::util::linux::LinuxFileManagers;
use anyhow::{Context, anyhow};
use async_fs::File;
use futures_util::{AsyncReadExt, io};
use log::error;
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::spawn;

pub async fn exists_file<P: AsRef<Path>>(path: P) -> io::Result<bool> {
    match File::open(path).await {
        Ok(_) => Ok(true),
        Err(e) => {
            if e.kind() == ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(e)
            }
        }
    }
}

pub async fn exists_folder<P: AsRef<Path>>(path: P) -> io::Result<bool> {
    if let Err(e) = async_fs::read_dir(path).await {
        if e.kind() == ErrorKind::NotFound {
            Ok(false)
        } else {
            Err(e)
        }
    } else {
        Ok(true)
    }
}

pub async fn delete_file_if_exists<P: AsRef<Path>>(path: P) -> io::Result<()> {
    if let Err(e) = async_fs::remove_file(path).await {
        if e.kind() == ErrorKind::NotFound {
            Ok(())
        } else {
            Err(e)
        }
    } else {
        Ok(())
    }
}

#[test]
fn test_exists_file() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let project_dir = env!("CARGO_MANIFEST_DIR");

    let cargo_file = PathBuf::new().join(project_dir).join("Cargo.toml");
    rt.block_on(async {
        assert_eq!(false, exists_file(Path::new("/foo/bar")).await.unwrap());
        assert_eq!(true, exists_file(&cargo_file).await.unwrap());
    });
}
#[test]
fn test_exists_folder() {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let project_dir = env!("CARGO_MANIFEST_DIR");

    rt.block_on(async {
        assert_eq!(false, exists_file(Path::new("/foo/bar")).await.unwrap());
        assert_eq!(
            true,
            exists_file(&PathBuf::new().join(project_dir))
                .await
                .unwrap()
        );
    });
}

pub async fn get_available_filename<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let path = path.as_ref();
    // 如果文件不存在，直接返回原路径
    if !exists_file(path).await? {
        return Ok(path.to_path_buf());
    }

    // 分离文件名和扩展名
    let parent = path.parent().unwrap_or(Path::new("."));
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let extension = path
        .extension()
        .map(|ext| format!(".{}", ext.to_string_lossy()))
        .unwrap_or_default();

    // 尝试添加序号
    let mut counter = 1;
    loop {
        let new_filename = format!("{}({}){}", stem, counter, extension);
        let new_path = parent.join(new_filename);

        if !exists_file(&new_path).await? {
            return Ok(new_path);
        }

        counter += 1;
    }
}

// 测试示例
#[test]
fn test_get_available_filename() {
    let project_dir = env!("CARGO_MANIFEST_DIR");
    let cargo_file = PathBuf::new().join(project_dir).join("Cargo.toml");

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    rt.block_on(async {
        // 测试自动重命名
        let path = get_available_filename(cargo_file).await.unwrap();
        println!("Available filename: {}", path.display());
    });
}

mod linux {
    use crate::util::linux::LinuxFileManagers::{
        Caja, Dolphin, Nautilus, Pcmanfm, Thunar, Unknown,
    };
    use std::process::Command;

    #[derive(Debug, Eq, PartialEq)]
    pub enum LinuxFileManagers {
        Nautilus,
        Dolphin,
        Thunar,
        Pcmanfm,
        Caja,
        Unknown,
    }
    pub fn get_default_file_manager() -> anyhow::Result<LinuxFileManagers> {
        let output = Command::new("xdg-mime")
            .args(&["query", "default", "inode/directory"])
            .output()?;

        if output.status.success() {
            let desktop_file_name = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if desktop_file_name.contains("Nautilus") {
                Ok(Nautilus)
            } else if desktop_file_name.contains("Dolphin") {
                Ok(Dolphin)
            } else if desktop_file_name.contains("Thunar") {
                Ok(Thunar)
            } else if desktop_file_name.contains("Pcmanfm") {
                Ok(Pcmanfm)
            } else if desktop_file_name.contains("Caja") {
                Ok(Caja)
            } else {
                Ok(Unknown)
            }
        } else {
            Ok(Unknown)
        }
    }
}

pub fn open_folder_and_select_file(folder_path: &str, file_name: &str) -> anyhow::Result<()> {
    let full_path = Path::new(folder_path).join(file_name);

    if cfg!(target_os = "windows") {
        Command::new("explorer")
            .args(&["/select,", &full_path.to_string_lossy()])
            .spawn()?;
        Ok(())
    } else if cfg!(target_os = "macos") {
        Command::new("open")
            .args(&["-R", &full_path.to_string_lossy()])
            .spawn()?;
        Ok(())
    } else if cfg!(target_os = "linux") {
        let fm = linux::get_default_file_manager()?;
        match fm {
            LinuxFileManagers::Nautilus => {
                // Try nautilus --select, if it fails, use gio
                let status = Command::new("nautilus")
                    .args(&["--select", &full_path.to_string_lossy()])
                    .status();

                if status.is_err() {
                    Command::new("gio")
                        .args(&[
                            "open",
                            folder_path,
                            "--select",
                            &full_path.to_string_lossy(),
                        ])
                        .spawn()?;
                }
                Ok(())
            }
            LinuxFileManagers::Dolphin => {
                let full_path = PathBuf::new().join(folder_path).join(file_name);
                let file = full_path.to_str().unwrap();
                Command::new("dolphin").args(&["--select", file]).spawn()?;
                Ok(())
            }
            LinuxFileManagers::Thunar => {
                // Thunar doest not support select file option.
                // So, we just open the folder
                Command::new("thunar").arg(folder_path).spawn()?;
                Ok(())
            }
            LinuxFileManagers::Pcmanfm => {
                // Pcmanfm doest not support select file option.
                // So, we just open the folder
                Command::new("pcmanfm").arg(folder_path).spawn()?;
                Ok(())
            }
            LinuxFileManagers::Caja => {
                let full_path = PathBuf::new().join(folder_path).join(file_name);
                let file = full_path.to_str().unwrap();
                Command::new("caja").args(&["--select", &file]).spawn()?;
                Ok(())
            }
            LinuxFileManagers::Unknown => Err(anyhow!(
                "Failed to open folder and select file, unknown file manager"
            )),
        }
    } else {
        Err(anyhow!(
            "Failed to open folder and select file, system not supported"
        ))
    }
}

pub fn open_file_in_file_manager(full_path: &str) {
    let path = PathBuf::new().join(full_path);
    let folder_path = path.parent();
    let file_name = path.file_name();
    if let Some(folder_path) = folder_path {
        if let Some(file_name) = file_name {
            let folder_path = folder_path.to_str().unwrap().to_string();
            let file_name = file_name.to_str().unwrap().to_string();
            spawn(move || open_folder_and_select_file(&folder_path, &file_name).unwrap());
        } else {
            error!("Failed to get file name of {}", full_path);
        }
    } else {
        error!("Failed to get folder path of {}", full_path);
    }
}

#[test]
fn test_get_default_file_manager_on_linux() {
    if let Ok(fm) = linux::get_default_file_manager() {
        println!("Default file manager: {:?}", fm);
    } else {
        eprintln!("Failed to detect file manager.");
    }
}

#[test]
fn test_open_folder_and_select_file() {
    let project_dir = env!("CARGO_MANIFEST_DIR");
    let file_name = "Cargo.toml";
    open_folder_and_select_file(project_dir, file_name).unwrap()
}

/// 异步计算文件的 SHA256 哈希值
pub async fn compute_file_hash<P: AsRef<Path>>(path: P) -> io::Result<String> {
    let mut file = File::open(path).await?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 8192]; // 8KB 缓冲区

    loop {
        let bytes_read = file.read(&mut buffer).await?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

pub async fn check_file_hash<P: AsRef<Path>>(path: P, expected_hash: &str) -> io::Result<bool> {
    let file_hash = compute_file_hash(path).await?;
    Ok(file_hash == expected_hash)
}

pub enum CheckFileResult {
    Valid,
    Invalid(String),
}

/// 检查指定的文件是否有效
/// 文件有效的条件：
/// 1.文件存在
/// 2.文件的哈希一致
pub async fn check_file<P: AsRef<Path>>(
    file_path: P,
    expected_hash: &str,
) -> anyhow::Result<CheckFileResult> {
    if !exists_file(&file_path).await? {
        return Ok(CheckFileResult::Invalid("File not found".to_string()));
    }

    let hash = compute_file_hash(&file_path)
        .await
        .context("Failed to check file hash")?;
    if hash == expected_hash {
        Ok(CheckFileResult::Valid)
    } else {
        Ok(CheckFileResult::Invalid(format!(
            "Hash mismatch, expected: {}, actual:{}",
            expected_hash, hash
        )))
    }
}
