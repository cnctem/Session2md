use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use url::Url;

fn validate_external_url(value: &str) -> Result<Url, String> {
    let parsed = Url::parse(value).map_err(|error| format!("Invalid URL: {error}"))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err("Only http and https URLs can be opened".to_string());
    }
    Ok(parsed)
}

#[tauri::command]
pub async fn open_external_url(app: AppHandle, url: String) -> Result<bool, String> {
    let parsed = validate_external_url(&url)?;
    app.opener()
        .open_url(parsed.as_str(), None::<String>)
        .map_err(|error| format!("Failed to open link: {error}"))?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::validate_external_url;

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
}
