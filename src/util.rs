use anyhow::Context;
use async_fs::File;
use futures_util::{AsyncReadExt, io};
use sha2::{Digest, Sha256};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

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

pub async fn delete_file_if_exists<P: AsRef<Path>>(path: P) -> io::Result<()> {
    match async_fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
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
