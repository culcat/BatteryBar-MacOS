use regex::Regex;
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub release_url: String,
    pub has_update: bool,
}

/// Checks GitHub Releases API for the latest release
pub fn check_for_updates() -> Result<UpdateInfo, String> {
    let current_version = env!("CARGO_PKG_VERSION").to_string();

    let output = Command::new("curl")
        .arg("-s")
        .arg("--max-time")
        .arg("5")
        .arg("-H")
        .arg("User-Agent: BatteryBar-Updater")
        .arg("https://api.github.com/repos/culcat/BatteryBar-MacOS/releases/latest")
        .output()
        .map_err(|e| format!("Failed to run curl: {e}"))?;

    if !output.status.success() {
        return Err("Network request to GitHub failed".to_string());
    }

    let body = String::from_utf8_lossy(&output.stdout);
    parse_github_release(&body, &current_version)
}

/// Parses tag_name and html_url from GitHub release JSON response
pub fn parse_github_release(json_body: &str, current_version: &str) -> Result<UpdateInfo, String> {
    let tag_re = Regex::new(r#""tag_name"\s*:\s*"v?([^"]+)""#).map_err(|e| e.to_string())?;
    let url_re = Regex::new(r#""html_url"\s*:\s*"([^"]+)""#).map_err(|e| e.to_string())?;

    let latest_tag = tag_re
        .captures(json_body)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .ok_or_else(|| "Could not find tag_name in response".to_string())?;

    let release_url = url_re
        .captures(json_body)
        .and_then(|caps| caps.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| {
            format!("https://github.com/culcat/BatteryBar-MacOS/releases/tag/v{latest_tag}")
        });

    let has_update = is_newer_version(&latest_tag, current_version);

    Ok(UpdateInfo {
        current_version: current_version.to_string(),
        latest_version: latest_tag,
        release_url,
        has_update,
    })
}

/// Opens the specified URL in the user's default browser
pub fn open_release_url(url: &str) {
    let _ = Command::new("open").arg(url).spawn();
}

/// Compares two semver strings: returns true if `remote` is strictly newer than `current`
pub fn is_newer_version(remote: &str, current: &str) -> bool {
    let parse_parts = |v: &str| -> Vec<u32> {
        v.trim_start_matches('v')
            .split('.')
            .filter_map(|s| s.parse::<u32>().ok())
            .collect()
    };

    let r_parts = parse_parts(remote);
    let c_parts = parse_parts(current);

    let max_len = r_parts.len().max(c_parts.len());
    for i in 0..max_len {
        let r = r_parts.get(i).copied().unwrap_or(0);
        let c = c_parts.get(i).copied().unwrap_or(0);
        if r > c {
            return true;
        } else if r < c {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("v0.1.2", "0.1.1"));
        assert!(is_newer_version("0.2.0", "0.1.1"));
        assert!(is_newer_version("1.0.0", "0.9.9"));
        assert!(!is_newer_version("0.1.1", "0.1.1"));
        assert!(!is_newer_version("0.1.0", "0.1.1"));
        assert!(!is_newer_version("0.1.1", "0.1.2"));
    }

    #[test]
    fn test_parse_github_release_newer() {
        let sample = r#"{
            "tag_name": "v0.2.0",
            "html_url": "https://github.com/culcat/BatteryBar-MacOS/releases/tag/v0.2.0"
        }"#;
        let info = parse_github_release(sample, "0.1.1").unwrap();
        assert_eq!(info.latest_version, "0.2.0");
        assert!(info.has_update);
        assert_eq!(
            info.release_url,
            "https://github.com/culcat/BatteryBar-MacOS/releases/tag/v0.2.0"
        );
    }

    #[test]
    fn test_parse_github_release_same() {
        let sample = r#"{
            "tag_name": "v0.1.1",
            "html_url": "https://github.com/culcat/BatteryBar-MacOS/releases/tag/v0.1.1"
        }"#;
        let info = parse_github_release(sample, "0.1.1").unwrap();
        assert_eq!(info.latest_version, "0.1.1");
        assert!(!info.has_update);
    }
}
