use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use crate::error::{CliError, Result};

const VERSION_URL: &str =
    "https://googlechromelabs.github.io/chrome-for-testing/LATEST_RELEASE_STABLE";

const DOWNLOAD_URL_TEMPLATE: &str =
    "https://storage.googleapis.com/chrome-for-testing-public/{VERSION}/linux64/chrome-linux64.zip";

const WELL_KNOWN_PATHS: &[&str] = &[
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
    "/usr/bin/google-chrome",
    "/usr/bin/google-chrome-stable",
];

const PATH_BINARY_NAMES: &[&str] = &["chromium", "chromium-browser", "google-chrome"];

const ZIP_DIR_PREFIX: &str = "chrome-linux64/";
const ZIP_CHROME_BINARY: &str = "chrome-linux64/chrome";

/// Resolves a Chrome/Chromium binary path, downloading if necessary.
///
/// Resolution order:
/// 1. If `config_path` is `Some`, return it immediately (trust user config).
/// 2. Check well-known filesystem paths (including `~/.local/searxng-cli/chromium/chrome-linux64/chrome`).
/// 3. Search `$PATH` for known binary names.
/// 4. Download Chrome for Testing to `~/.local/searxng-cli/chromium/`.
pub async fn resolve_chrome_path(config_path: Option<&str>) -> Result<String> {
    if let Some(path) = config_path {
        return Ok(path.to_string());
    }

    if let Some(path) = find_in_well_known_paths() {
        return Ok(path);
    }

    if let Some(path) = find_in_path() {
        return Ok(path);
    }

    eprintln!("Chromium not found locally, downloading Chrome for Testing...");
    let dest = download_and_install().await?;
    eprintln!("Chrome for Testing installed successfully at {dest}");
    Ok(dest)
}

fn install_dir() -> Result<PathBuf> {
    let home = std::env::var("HOME")
        .map_err(|_| CliError::Browser("Cannot determine home directory".to_string()))?;
    Ok(PathBuf::from(home).join(".local").join("searxng-cli").join("chromium"))
}

fn installed_chrome_path() -> Result<PathBuf> {
    Ok(install_dir()?.join("chrome-linux64").join("chrome"))
}

fn find_in_well_known_paths() -> Option<String> {
    for path in WELL_KNOWN_PATHS {
        if is_executable(Path::new(path)) {
            return Some(path.to_string());
        }
    }

    if let Ok(local_path) = installed_chrome_path() {
        if is_executable(&local_path) {
            return Some(local_path.to_string_lossy().to_string());
        }
    }

    None
}

fn find_in_path() -> Option<String> {
    let path_var = std::env::var("PATH").ok()?;

    for dir in path_var.split(':') {
        let dir_path = Path::new(dir);
        for name in PATH_BINARY_NAMES {
            let candidate = dir_path.join(name);
            if is_executable(&candidate) {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    None
}

fn is_executable(path: &Path) -> bool {
    match fs::metadata(path) {
        Ok(meta) => meta.is_file() && (meta.permissions().mode() & 0o111 != 0),
        Err(_) => false,
    }
}

async fn download_and_install() -> Result<String> {
    let version = fetch_latest_version().await?;
    eprintln!("Detected latest stable version: {version}");

    let url = DOWNLOAD_URL_TEMPLATE.replace("{VERSION}", &version);
    eprintln!("Downloading chrome-linux64.zip...");

    let zip_bytes = download_chrome(&url).await?;

    let dest_dir = install_dir()?;
    extract_chrome_archive(&zip_bytes, &dest_dir)?;

    let chrome_binary = dest_dir.join("chrome-linux64").join("chrome");
    Ok(chrome_binary.to_string_lossy().to_string())
}

async fn fetch_latest_version() -> Result<String> {
    let resp = reqwest::get(VERSION_URL)
        .await
        .map_err(|e| CliError::Browser(format!("Failed to fetch Chrome version: {e}")))?;

    if !resp.status().is_success() {
        return Err(CliError::Browser(format!(
            "Failed to fetch Chrome version: HTTP {}",
            resp.status()
        )));
    }

    let text = resp
        .text()
        .await
        .map_err(|e| CliError::Browser(format!("Failed to read Chrome version response: {e}")))?;

    let version = text.trim().to_string();

    if version.is_empty() || !version.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return Err(CliError::Browser(format!(
            "Invalid Chrome version string: {version:?}"
        )));
    }

    Ok(version)
}

async fn download_chrome(url: &str) -> Result<Vec<u8>> {
    let resp = reqwest::get(url)
        .await
        .map_err(|e| CliError::Browser(format!("Failed to download Chrome: {e}")))?;

    if !resp.status().is_success() {
        return Err(CliError::Browser(format!(
            "Failed to download Chrome: HTTP {}",
            resp.status()
        )));
    }

    let content_length = resp.content_length();
    if let Some(len) = content_length {
        eprintln!("Download size: {:.1} MB", len as f64 / 1_048_576.0);
    }

    let bytes = resp
        .bytes()
        .await
        .map_err(|e| CliError::Browser(format!("Failed to read Chrome download: {e}")))?;

    Ok(bytes.to_vec())
}

fn extract_chrome_archive(zip_bytes: &[u8], dest_dir: &Path) -> Result<()> {
    let cursor = io::Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)
        .map_err(|e| CliError::Browser(format!("Failed to open Chrome archive: {e}")))?;

    let chrome_dir = dest_dir.join("chrome-linux64");
    if chrome_dir.exists() {
        fs::remove_dir_all(&chrome_dir).map_err(|e| {
            CliError::Browser(format!("Failed to remove old Chrome installation: {e}"))
        })?;
    }

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).map_err(|e| {
            CliError::Browser(format!("Failed to read archive entry: {e}"))
        })?;

        let name = file.name().to_string();
        if !name.starts_with(ZIP_DIR_PREFIX) {
            continue;
        }

        let out_path = dest_dir.join(&name);

        if name.ends_with('/') {
            fs::create_dir_all(&out_path).map_err(|e| {
                CliError::Browser(format!("Failed to create directory {}: {e}", out_path.display()))
            })?;
        } else {
            if let Some(parent) = out_path.parent() {
                fs::create_dir_all(parent).map_err(|e| {
                    CliError::Browser(format!(
                        "Failed to create directory {}: {e}",
                        parent.display()
                    ))
                })?;
            }

            let mut out_file = fs::File::create(&out_path).map_err(|e| {
                CliError::Browser(format!("Failed to create file {}: {e}", out_path.display()))
            })?;

            io::copy(&mut file, &mut out_file).map_err(|e| {
                CliError::Browser(format!("Failed to extract {}: {e}", name))
            })?;

            if let Some(mode) = file.unix_mode() {
                fs::set_permissions(&out_path, fs::Permissions::from_mode(mode)).ok();
            } else if name == ZIP_CHROME_BINARY {
                fs::set_permissions(&out_path, fs::Permissions::from_mode(0o755)).map_err(|e| {
                    CliError::Browser(format!(
                        "Failed to set executable permissions: {e}"
                    ))
                })?;
            }
        }
    }

    let chrome_path = dest_dir.join(ZIP_CHROME_BINARY);
    if !chrome_path.exists() {
        return Err(CliError::Browser(
            "Chrome binary not found after extraction".to_string(),
        ));
    }

    eprintln!("Extracted to {}", chrome_dir.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_is_executable_nonexistent() {
        assert!(!is_executable(Path::new("/nonexistent/path/binary")));
    }

    #[test]
    fn test_is_executable_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_bin");
        fs::write(&file_path, b"#!/bin/sh\n").unwrap();
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(is_executable(&file_path));
    }

    #[test]
    fn test_is_executable_non_executable_file() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_file");
        fs::write(&file_path, b"data").unwrap();
        fs::set_permissions(&file_path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(!is_executable(&file_path));
    }

    #[test]
    fn test_is_executable_directory() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!is_executable(dir.path()));
    }

    #[test]
    fn test_find_in_path_with_custom_path() {
        let dir = tempfile::tempdir().unwrap();
        let bin_path = dir.path().join("chromium");
        fs::write(&bin_path, b"#!/bin/sh\n").unwrap();
        fs::set_permissions(&bin_path, fs::Permissions::from_mode(0o755)).unwrap();

        let original_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", dir.path().to_str().unwrap());

        let result = find_in_path();
        assert_eq!(result, Some(bin_path.to_string_lossy().to_string()));

        std::env::set_var("PATH", &original_path);
    }

    #[test]
    fn test_find_in_path_not_found() {
        let original_path = std::env::var("PATH").unwrap_or_default();
        std::env::set_var("PATH", "/nonexistent_dir_xyz");

        let result = find_in_path();
        assert!(result.is_none());

        std::env::set_var("PATH", &original_path);
    }

    #[tokio::test]
    async fn test_resolve_chrome_path_config_takes_precedence() {
        let result = resolve_chrome_path(Some("/custom/chrome")).await.unwrap();
        assert_eq!(result, "/custom/chrome");
    }

    #[test]
    fn test_installed_chrome_path() {
        let original_home = std::env::var("HOME").unwrap_or_default();
        std::env::set_var("HOME", "/home/testuser");

        let path = installed_chrome_path().unwrap();
        assert_eq!(
            path,
            PathBuf::from("/home/testuser/.local/searxng-cli/chromium/chrome-linux64/chrome")
        );

        std::env::set_var("HOME", &original_home);
    }

    #[test]
    fn test_extract_chrome_archive_valid_zip() {
        let mut zip_buf = Vec::new();
        {
            let cursor = io::Cursor::new(&mut zip_buf);
            let mut zip_writer = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);

            zip_writer
                .start_file("chrome-linux64/chrome", options)
                .unwrap();
            zip_writer.write_all(b"#!/bin/sh\necho chrome\n").unwrap();

            zip_writer
                .start_file("chrome-linux64/icudtl.dat", options)
                .unwrap();
            zip_writer.write_all(b"fake icu data").unwrap();

            zip_writer.finish().unwrap();
        }

        let dir = tempfile::tempdir().unwrap();
        extract_chrome_archive(&zip_buf, dir.path()).unwrap();

        let chrome_path = dir.path().join("chrome-linux64").join("chrome");
        assert!(chrome_path.exists());

        let icu_path = dir.path().join("chrome-linux64").join("icudtl.dat");
        assert!(icu_path.exists());

        use std::io::Read;
        let mut content = String::new();
        fs::File::open(&chrome_path)
            .unwrap()
            .read_to_string(&mut content)
            .unwrap();
        assert_eq!(content, "#!/bin/sh\necho chrome\n");
    }

    #[test]
    fn test_extract_chrome_archive_missing_binary() {
        let mut zip_buf = Vec::new();
        {
            let cursor = io::Cursor::new(&mut zip_buf);
            let mut zip_writer = zip::ZipWriter::new(cursor);
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            zip_writer
                .start_file("wrong-path/chrome", options)
                .unwrap();
            zip_writer.write_all(b"data").unwrap();
            zip_writer.finish().unwrap();
        }

        let dir = tempfile::tempdir().unwrap();
        let result = extract_chrome_archive(&zip_buf, dir.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("not found after extraction"));
    }

    #[test]
    fn test_extract_chrome_archive_invalid_zip() {
        let dir = tempfile::tempdir().unwrap();
        let result = extract_chrome_archive(b"not a zip file", dir.path());
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to open Chrome archive"));
    }

    #[tokio::test]
    async fn test_fetch_latest_version_validation() {
        let version = "151.0.7922.47";
        assert!(version.chars().all(|c| c.is_ascii_digit() || c == '.'));

        let bad_version = "not-a-version!";
        assert!(!bad_version.chars().all(|c| c.is_ascii_digit() || c == '.'));
    }
}
