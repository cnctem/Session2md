use semver::Version;
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

const LATEST_RELEASE_URL: &str = "https://api.github.com/repos/cnctem/Session2md/releases/latest";
const USER_AGENT: &str = concat!("Session2md/", env!("CARGO_PKG_VERSION"));

#[derive(Debug, Deserialize)]
struct GitHubRelease {
    tag_name: String,
    html_url: String,
    name: Option<String>,
    published_at: Option<String>,
    #[serde(default)]
    assets: Vec<GitHubAsset>,
}

#[derive(Debug, Deserialize)]
struct GitHubAsset {
    name: String,
    browser_download_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdateInfo {
    latest_version: String,
    release_name: Option<String>,
    release_url: String,
    download_url: String,
    published_at: Option<String>,
}

fn normalized_version(value: &str) -> &str {
    value.trim().trim_start_matches(['v', 'V'])
}

fn parse_version(value: &str) -> Result<Version, String> {
    Version::parse(normalized_version(value))
        .map_err(|error| format!("无法解析版本号 {value}: {error}"))
}

fn current_platform_asset_suffix() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("macos", _) => Some("-macOS.dmg"),
        ("windows", "aarch64") => Some("-Windows-arm64.msi"),
        ("windows", _) => Some("-Windows.msi"),
        ("linux", "aarch64") => Some("-Linux-arm64.AppImage"),
        ("linux", _) => Some("-Linux-x86_64.AppImage"),
        _ => None,
    }
}

fn select_download_asset(release: &GitHubRelease) -> Option<&GitHubAsset> {
    let suffix = current_platform_asset_suffix()?;
    release
        .assets
        .iter()
        .find(|asset| asset.name.ends_with(suffix))
}

fn build_update_info(
    app: &AppHandle,
    release: GitHubRelease,
) -> Result<Option<AppUpdateInfo>, String> {
    // Release builds stamp tauri.conf.json from the Git tag before compiling,
    // so package_info is the tag-derived current version in shipped binaries.
    let current_version = app.package_info().version.to_string();
    let latest = parse_version(&release.tag_name)?;

    if latest <= parse_version(&current_version)? {
        return Ok(None);
    }

    let selected_asset = select_download_asset(&release);
    let download_url = selected_asset
        .map(|asset| asset.browser_download_url.clone())
        .unwrap_or_else(|| release.html_url.clone());
    let latest_version = latest.to_string();

    Ok(Some(AppUpdateInfo {
        latest_version,
        release_name: release.name.filter(|name| !name.trim().is_empty()),
        download_url,
        release_url: release.html_url,
        published_at: release.published_at,
    }))
}

/// Checks the latest stable GitHub Release and returns update details when the
/// release is newer than the running app. Downloads always point at GitHub.
#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<Option<AppUpdateInfo>, String> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|error| format!("初始化更新检查失败: {error}"))?;

    let response = client
        .get(LATEST_RELEASE_URL)
        .header("Accept", "application/vnd.github+json")
        .header("X-GitHub-Api-Version", "2022-11-28")
        .send()
        .await
        .map_err(|error| format!("检查更新失败: {error}"))?;

    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Ok(None);
    }

    if !response.status().is_success() {
        let status = response.status();
        let detail = response.text().await.unwrap_or_default();
        return Err(format!("GitHub Releases 请求失败 ({status}): {detail}"));
    }

    let release = response
        .json::<GitHubRelease>()
        .await
        .map_err(|error| format!("解析更新信息失败: {error}"))?;

    build_update_info(&app, release)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn release_with_assets(tag: &str, names: &[&str]) -> GitHubRelease {
        GitHubRelease {
            tag_name: tag.to_string(),
            html_url: format!("https://github.com/cnctem/Session2md/releases/tag/{tag}"),
            name: Some(tag.to_string()),
            published_at: None,
            assets: names
                .iter()
                .map(|name| GitHubAsset {
                    name: (*name).to_string(),
                    browser_download_url: format!("https://example.com/{name}"),
                })
                .collect(),
        }
    }

    #[test]
    fn normalizes_release_tags_with_optional_v_prefix() {
        assert_eq!(normalized_version("v2.3.1"), "2.3.1");
        assert_eq!(normalized_version(" 2.3.1 "), "2.3.1");
    }

    #[test]
    fn semantic_versions_drive_update_decisions() {
        assert!(parse_version("v2.3.1").unwrap() > parse_version("2.3.0").unwrap());
        assert!(parse_version("2.2.2").unwrap() < parse_version("2.3.0").unwrap());
    }

    #[test]
    fn selects_expected_platform_asset() {
        let release = release_with_assets(
            "2.3.1",
            &[
                "Session2md-2.3.1-macOS.zip",
                "Session2md-2.3.1-macOS.dmg",
                "Session2md-2.3.1-Windows.msi",
                "Session2md-2.3.1-Windows-arm64.msi",
                "Session2md-2.3.1-Linux-x86_64.AppImage",
                "Session2md-2.3.1-Linux-arm64.AppImage",
            ],
        );

        let asset = select_download_asset(&release).unwrap();
        match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", _) => assert_eq!(asset.name, "Session2md-2.3.1-macOS.dmg"),
            ("windows", "aarch64") => {
                assert_eq!(asset.name, "Session2md-2.3.1-Windows-arm64.msi")
            }
            ("windows", _) => assert_eq!(asset.name, "Session2md-2.3.1-Windows.msi"),
            ("linux", "aarch64") => {
                assert_eq!(asset.name, "Session2md-2.3.1-Linux-arm64.AppImage")
            }
            ("linux", _) => {
                assert_eq!(asset.name, "Session2md-2.3.1-Linux-x86_64.AppImage")
            }
            _ => {}
        }
    }
}
