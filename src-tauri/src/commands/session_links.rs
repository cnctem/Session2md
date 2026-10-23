#![allow(non_snake_case)]

use std::fs;
use std::path::{Path, PathBuf};

use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use url::Url;

/// Keep inline image payloads reasonable: the bytes are base64 encoded and sent
/// across the Tauri IPC boundary, so very large files would stall the UI.
const MAX_IMAGE_BYTES: u64 = 12 * 1024 * 1024;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalImagePayload {
    pub path: String,
    pub mime: String,
    pub data_url: String,
}

/// Structured command error so the frontend can translate known failures and
/// show the resolved path instead of a raw backend message.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalFileError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
}

impl LocalFileError {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            path: None,
        }
    }

    fn with_path(mut self, path: &Path) -> Self {
        self.path = Some(path.to_string_lossy().into_owned());
        self
    }

    fn file_not_found(path: &Path) -> Self {
        Self::new(
            "fileNotFound",
            format!("File not found: {}", path.display()),
        )
        .with_path(path)
    }
}

impl From<String> for LocalFileError {
    fn from(message: String) -> Self {
        Self::new("localFileError", message)
    }
}

fn validate_external_url(value: &str) -> Result<Url, String> {
    let parsed = Url::parse(value).map_err(|error| format!("Invalid URL: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only http and https URLs can be opened".to_string());
    }
    Ok(parsed)
}

fn strip_wrapping(value: &str) -> &str {
    let trimmed = value.trim();
    if trimmed.len() >= 2 && trimmed.starts_with('<') && trimmed.ends_with('>') {
        &trimmed[1..trimmed.len() - 1]
    } else {
        trimmed
    }
}

fn expand_home(value: &str) -> PathBuf {
    if value == "~" {
        if let Some(home) = dirs::home_dir() {
            return home;
        }
    } else if let Some(rest) = value
        .strip_prefix("~/")
        .or_else(|| value.strip_prefix("~\\"))
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(value)
}

/// Resolve a Markdown link/image target into an absolute local path.
///
/// `file://` URLs, absolute paths and `~` are used as-is. Relative paths are
/// resolved against `base_dir`, which is the session working directory.
fn resolve_local_path(raw: &str, base_dir: Option<&str>) -> Result<PathBuf, LocalFileError> {
    let value = strip_wrapping(raw);
    if value.is_empty() {
        return Err(LocalFileError::new("emptyPath", "Path is empty"));
    }

    let path = if value.to_ascii_lowercase().starts_with("file:") {
        let parsed = Url::parse(value).map_err(|error| format!("Invalid file URL: {error}"))?;
        parsed
            .to_file_path()
            .map_err(|_| LocalFileError::new("invalidFileUrl", "Invalid file URL"))?
    } else {
        let candidate = expand_home(value);
        if candidate.is_absolute() {
            candidate
        } else {
            match base_dir.map(str::trim).filter(|dir| !dir.is_empty()) {
                Some(dir) => expand_home(dir).join(candidate),
                None => {
                    return Err(LocalFileError::new(
                        "missingBaseDir",
                        "Cannot resolve a relative path without a session directory",
                    ))
                }
            }
        }
    };

    std::path::absolute(&path)
        .map_err(|error| LocalFileError::new("invalidPath", format!("Invalid path: {error}")))
}

fn ensure_exists(path: &Path) -> Result<fs::Metadata, LocalFileError> {
    match fs::metadata(path) {
        Ok(metadata) => Ok(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Err(LocalFileError::file_not_found(path))
        }
        Err(error) => Err(LocalFileError::new(
            "fileAccessFailed",
            format!("Failed to access file: {error}"),
        )
        .with_path(path)),
    }
}

fn image_mime(path: &Path) -> Option<&'static str> {
    let extension = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "bmp" => "image/bmp",
        "svg" => "image/svg+xml",
        "avif" => "image/avif",
        "ico" => "image/x-icon",
        _ => return None,
    })
}

/// Documents that are safe and useful to hand off to the default application.
/// Anything else is revealed in the file manager instead of being executed.
fn opens_in_default_app(path: &Path) -> bool {
    let Some(extension) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "pdf"
            | "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "svg"
            | "avif"
            | "ico"
            | "txt"
            | "md"
            | "markdown"
            | "json"
            | "csv"
            | "log"
            | "yaml"
            | "yml"
            | "toml"
            | "mp4"
            | "mov"
            | "webm"
            | "m4v"
            | "mp3"
            | "wav"
            | "m4a"
            | "pptx"
            | "ppt"
            | "docx"
            | "doc"
            | "xlsx"
            | "xls"
            | "key"
            | "pages"
            | "numbers"
    )
}

fn read_local_image_sync(
    path: &str,
    base_dir: Option<&str>,
) -> Result<LocalImagePayload, LocalFileError> {
    let resolved = resolve_local_path(path, base_dir)?;
    let metadata = ensure_exists(&resolved)?;
    if !metadata.is_file() {
        return Err(
            LocalFileError::new("notAFile", format!("Not a file: {}", resolved.display()))
                .with_path(&resolved),
        );
    }
    let mime = image_mime(&resolved).ok_or_else(|| {
        LocalFileError::new(
            "unsupportedImageType",
            format!("Unsupported image type: {}", resolved.display()),
        )
        .with_path(&resolved)
    })?;
    if metadata.len() > MAX_IMAGE_BYTES {
        return Err(LocalFileError::new(
            "imageTooLarge",
            format!(
                "Image is larger than {} MB: {}",
                MAX_IMAGE_BYTES / (1024 * 1024),
                resolved.display()
            ),
        )
        .with_path(&resolved));
    }

    let bytes = fs::read(&resolved).map_err(|error| {
        LocalFileError::new("readFailed", format!("Failed to read image: {error}"))
            .with_path(&resolved)
    })?;
    let data_url = format!("data:{mime};base64,{}", STANDARD.encode(bytes));

    Ok(LocalImagePayload {
        path: resolved.to_string_lossy().into_owned(),
        mime: mime.to_string(),
        data_url,
    })
}

#[tauri::command]
pub async fn open_external_url(app: AppHandle, url: String) -> Result<bool, String> {
    let parsed = validate_external_url(&url)?;
    app.opener()
        .open_url(parsed.as_str(), None::<String>)
        .map_err(|error| format!("Failed to open link: {error}"))?;
    Ok(true)
}

/// Open a local path referenced from a conversation. Relative paths are resolved
/// against the session working directory.
#[tauri::command]
pub async fn open_local_path(
    app: AppHandle,
    path: String,
    baseDir: Option<String>,
) -> Result<String, LocalFileError> {
    let resolved = resolve_local_path(&path, baseDir.as_deref())?;
    let metadata = ensure_exists(&resolved)?;
    let display = resolved.to_string_lossy().into_owned();

    if metadata.is_dir() || opens_in_default_app(&resolved) {
        app.opener()
            .open_path(display.clone(), None::<String>)
            .map_err(|error| {
                LocalFileError::new("openFailed", format!("Failed to open path: {error}"))
                    .with_path(&resolved)
            })?;
    } else {
        app.opener()
            .reveal_item_in_dir(&resolved)
            .map_err(|error| {
                LocalFileError::new("revealFailed", format!("Failed to reveal path: {error}"))
                    .with_path(&resolved)
            })?;
    }

    Ok(display)
}

#[tauri::command]
pub async fn read_local_image(
    path: String,
    baseDir: Option<String>,
) -> Result<LocalImagePayload, LocalFileError> {
    tauri::async_runtime::spawn_blocking(move || read_local_image_sync(&path, baseDir.as_deref()))
        .await
        .map_err(|error| {
            LocalFileError::new("readFailed", format!("Failed to read image: {error}"))
        })?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_http_and_https_urls() {
        assert!(validate_external_url("https://example.com/docs").is_ok());
        assert!(validate_external_url("http://localhost:3000").is_ok());
    }

    #[test]
    fn rejects_non_web_or_invalid_urls() {
        for value in [
            "javascript:alert(1)",
            "file:///tmp/secret",
            "mailto:user@example.com",
            "not a url",
        ] {
            assert!(validate_external_url(value).is_err(), "{value}");
        }
    }

    #[test]
    fn resolves_relative_paths_against_the_session_directory() {
        let resolved =
            resolve_local_path("video/cinema/output/封面.png", Some("/Users/example/wic"))
                .expect("relative path should resolve");
        assert_eq!(
            resolved,
            PathBuf::from("/Users/example/wic/video/cinema/output/封面.png")
        );
    }

    #[test]
    fn resolves_absolute_paths_without_a_session_directory() {
        let resolved = resolve_local_path("/Users/example/wic/docs/deck/汇报.pdf", None)
            .expect("absolute path should resolve");
        assert_eq!(
            resolved,
            PathBuf::from("/Users/example/wic/docs/deck/汇报.pdf")
        );
    }

    #[test]
    fn resolves_file_urls_and_decoded_unicode_paths() {
        let resolved =
            resolve_local_path("file:///tmp/a%20b.png", None).expect("file URL should resolve");
        assert_eq!(resolved, PathBuf::from("/tmp/a b.png"));

        let resolved =
            resolve_local_path("docs/cabin-library.png", Some("/tmp/wic")).expect("resolve");
        assert_eq!(resolved, PathBuf::from("/tmp/wic/docs/cabin-library.png"));
    }

    #[test]
    fn rejects_relative_paths_without_a_session_directory() {
        let error = resolve_local_path("docs/a.png", None)
            .expect_err("relative path without a session directory should fail");
        assert_eq!(error.code, "missingBaseDir");
    }

    #[test]
    fn only_image_extensions_are_inline_readable() {
        assert_eq!(image_mime(Path::new("a.PNG")), Some("image/png"));
        assert_eq!(image_mime(Path::new("a.svg")), Some("image/svg+xml"));
        assert_eq!(image_mime(Path::new("a.pdf")), None);
    }

    #[test]
    fn reads_images_as_data_urls_and_reports_the_resolved_path() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("sample.png");
        fs::write(&path, [0x89, b'P', b'N', b'G']).expect("write image");

        let payload = read_local_image_sync("sample.png", Some(dir.path().to_str().unwrap()))
            .expect("read image");
        assert_eq!(payload.mime, "image/png");
        assert_eq!(payload.path, path.to_string_lossy());
        assert!(payload.data_url.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn refuses_to_inline_non_image_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("notes.txt");
        fs::write(&path, b"hello").expect("write file");

        assert!(read_local_image_sync(path.to_str().unwrap(), None).is_err());
    }

    #[test]
    fn serializes_missing_files_with_a_code_and_path() {
        let error = LocalFileError::file_not_found(Path::new("/tmp/missing.png"));
        let value = serde_json::to_value(error).expect("serialize error");

        assert_eq!(value["code"], "fileNotFound");
        assert_eq!(value["path"], "/tmp/missing.png");
        assert!(value["message"]
            .as_str()
            .expect("message")
            .contains("missing.png"));
    }

    #[test]
    fn reports_missing_files_as_structured_errors() {
        let dir = tempfile::tempdir().expect("temp dir");
        let missing = dir.path().join("missing.png");
        let error = read_local_image_sync(missing.to_str().unwrap(), None)
            .expect_err("missing file should fail");

        assert_eq!(error.code, "fileNotFound");
        let expected = missing.to_string_lossy().into_owned();
        assert_eq!(error.path.as_deref(), Some(expected.as_str()));
        assert!(error.message.contains("missing.png"));
    }
}
