use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;

pub const DEFAULT_UPDATER_PUBKEY: &str = "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDJBRjZCREYyRkM4QUU4NEIKUldSTDZJcjg4cjMyS3ZDb3d0Ry9adnE5TWoyN1FMeSsyMEFTUDdObVNtQnZuR1dnV3EwSVNETy8K";

pub const STABLE_UPDATER_ENDPOINT: &str =
    "https://github.com/AirSodaz/sona/releases/latest/download/updater.json";

pub const NIGHTLY_UPDATER_ENDPOINT: &str =
    "https://github.com/AirSodaz/sona/releases/download/nightly/updater.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpdateChannel {
    Stable,
    Nightly,
}

impl fmt::Display for UpdateChannel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stable => write!(f, "stable"),
            Self::Nightly => write!(f, "nightly"),
        }
    }
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum AppUpdateError {
    #[error("Unsupported platform: {os}-{arch}")]
    UnsupportedPlatform { os: String, arch: String },
    #[error("Unknown update channel: '{0}'. Expected 'stable' or 'nightly'")]
    UnknownChannel(String),
}

impl FromStr for UpdateChannel {
    type Err = AppUpdateError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "stable" => Ok(Self::Stable),
            "nightly" => Ok(Self::Nightly),
            other => Err(AppUpdateError::UnknownChannel(other.to_string())),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VersionComparison {
    Newer,
    Same,
    Older,
}

impl VersionComparison {
    #[must_use]
    pub fn is_upgrade(&self) -> bool {
        matches!(self, Self::Newer)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppUpdateManifest {
    pub version: String,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub pub_date: Option<String>,
    pub platforms: HashMap<String, AppPlatformAsset>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppPlatformAsset {
    pub url: String,
    pub signature: String,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct AppUpdateCheckResult {
    pub update_available: bool,
    pub current_version: String,
    pub latest_version: String,
    pub channel: UpdateChannel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pub_date: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
    pub release_url: String,
    pub asset_url: String,
    #[serde(default)]
    pub signature: String,
    pub platform: String,
}

#[must_use]
pub fn updater_endpoint_for_channel(channel: UpdateChannel) -> &'static str {
    match channel {
        UpdateChannel::Stable => STABLE_UPDATER_ENDPOINT,
        UpdateChannel::Nightly => NIGHTLY_UPDATER_ENDPOINT,
    }
}

#[must_use]
pub fn release_url_for_version(version: &str, channel: UpdateChannel) -> String {
    match channel {
        UpdateChannel::Nightly => {
            "https://github.com/AirSodaz/sona/releases/tag/nightly".to_string()
        }
        UpdateChannel::Stable => {
            let clean_v = version.trim().trim_start_matches(['v', 'V']);
            format!("https://github.com/AirSodaz/sona/releases/tag/v{clean_v}")
        }
    }
}

pub fn platform_key(os: &str, arch: &str) -> Result<&'static str, AppUpdateError> {
    match (os, arch) {
        ("macos" | "darwin", "aarch64") => Ok("darwin-aarch64"),
        ("macos" | "darwin", "x86_64") => Ok("darwin-x86_64"),
        ("windows", "x86_64") => Ok("windows-x86_64"),
        ("windows", "aarch64") => Ok("windows-aarch64"),
        ("linux", "x86_64") => Ok("linux-x86_64"),
        ("linux", "aarch64") => Ok("linux-aarch64"),
        _ => Err(AppUpdateError::UnsupportedPlatform {
            os: os.to_string(),
            arch: arch.to_string(),
        }),
    }
}

pub fn current_platform_key() -> Result<&'static str, AppUpdateError> {
    platform_key(std::env::consts::OS, std::env::consts::ARCH)
}

#[must_use]
pub fn parse_version_lenient(s: &str) -> Option<semver::Version> {
    let s = s.trim().trim_start_matches(['v', 'V']);
    if s.is_empty() {
        return None;
    }
    if let Ok(v) = semver::Version::parse(s) {
        return Some(v);
    }
    let (core, rest) = match s.find(['-', '+']) {
        Some(idx) => (&s[..idx], &s[idx..]),
        None => (s, ""),
    };
    let parts: Vec<&str> = core.split('.').collect();
    let normalized = match parts.as_slice() {
        [major] => {
            major.parse::<u64>().ok()?;
            format!("{major}.0.0{rest}")
        }
        [major, minor] => {
            major.parse::<u64>().ok()?;
            minor.parse::<u64>().ok()?;
            format!("{major}.{minor}.0{rest}")
        }
        _ => return None,
    };
    semver::Version::parse(&normalized).ok()
}

#[must_use]
pub fn parse_semver(s: &str) -> Option<(u64, u64, u64)> {
    parse_version_lenient(s).map(|v| (v.major, v.minor, v.patch))
}

fn extract_date_components(s: &str) -> Option<(u32, u32, u32)> {
    for window in s.as_bytes().windows(10) {
        let is_sep1 = window[4] == b'-' || window[4] == b'.';
        let is_sep2 = window[7] == b'-' || window[7] == b'.';
        if is_sep1 && is_sep2 {
            let y_digits = window[0..4].iter().all(|b| b.is_ascii_digit());
            let m_digits = window[5..7].iter().all(|b| b.is_ascii_digit());
            let d_digits = window[8..10].iter().all(|b| b.is_ascii_digit());
            if y_digits && m_digits && d_digits {
                let y = std::str::from_utf8(&window[0..4])
                    .ok()?
                    .parse::<u32>()
                    .ok()?;
                let m = std::str::from_utf8(&window[5..7])
                    .ok()?
                    .parse::<u32>()
                    .ok()?;
                let d = std::str::from_utf8(&window[8..10])
                    .ok()?
                    .parse::<u32>()
                    .ok()?;
                if (1..=12).contains(&m) && (1..=31).contains(&d) {
                    return Some((y, m, d));
                }
            }
        }
    }
    None
}

#[must_use]
pub fn compare_versions(current: &str, target: &str, channel: UpdateChannel) -> VersionComparison {
    if current.trim() == target.trim() {
        return VersionComparison::Same;
    }

    match channel {
        UpdateChannel::Stable => {
            match (
                parse_version_lenient(current),
                parse_version_lenient(target),
            ) {
                (Some(c), Some(t)) => match t.cmp(&c) {
                    std::cmp::Ordering::Greater => VersionComparison::Newer,
                    std::cmp::Ordering::Less => VersionComparison::Older,
                    std::cmp::Ordering::Equal => VersionComparison::Same,
                },
                _ => match target.trim().cmp(current.trim()) {
                    std::cmp::Ordering::Greater => VersionComparison::Newer,
                    std::cmp::Ordering::Less => VersionComparison::Older,
                    std::cmp::Ordering::Equal => VersionComparison::Same,
                },
            }
        }
        UpdateChannel::Nightly => {
            let cur_date = extract_date_components(current);
            let tgt_date = extract_date_components(target);
            match (cur_date, tgt_date) {
                (Some(c), Some(t)) => match t.cmp(&c) {
                    std::cmp::Ordering::Greater => VersionComparison::Newer,
                    std::cmp::Ordering::Less => VersionComparison::Older,
                    std::cmp::Ordering::Equal => VersionComparison::Same,
                },
                (None, Some(_)) => VersionComparison::Newer,
                (Some(_), None) => VersionComparison::Older,
                (None, None) => {
                    match (
                        parse_version_lenient(current),
                        parse_version_lenient(target),
                    ) {
                        (Some(c), Some(t)) => {
                            if c.pre.is_empty()
                                && !t.pre.is_empty()
                                && c.major == t.major
                                && c.minor == t.minor
                                && c.patch == t.patch
                            {
                                VersionComparison::Newer
                            } else {
                                match t.cmp(&c) {
                                    std::cmp::Ordering::Greater => VersionComparison::Newer,
                                    std::cmp::Ordering::Less => VersionComparison::Older,
                                    std::cmp::Ordering::Equal => VersionComparison::Same,
                                }
                            }
                        }
                        _ => match target.trim().cmp(current.trim()) {
                            std::cmp::Ordering::Greater => VersionComparison::Newer,
                            std::cmp::Ordering::Less => VersionComparison::Older,
                            std::cmp::Ordering::Equal => VersionComparison::Same,
                        },
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_key_maps_supported_architectures() {
        assert_eq!(platform_key("windows", "x86_64").unwrap(), "windows-x86_64");
        assert_eq!(
            platform_key("windows", "aarch64").unwrap(),
            "windows-aarch64"
        );
        assert_eq!(platform_key("macos", "aarch64").unwrap(), "darwin-aarch64");
        assert_eq!(platform_key("darwin", "x86_64").unwrap(), "darwin-x86_64");
        assert_eq!(platform_key("linux", "x86_64").unwrap(), "linux-x86_64");
        assert_eq!(platform_key("linux", "aarch64").unwrap(), "linux-aarch64");
        assert!(platform_key("freebsd", "x86_64").is_err());
    }

    #[test]
    fn current_platform_key_returns_ok_on_supported_os() {
        let res = current_platform_key();
        assert!(
            res.is_ok(),
            "Expected current platform to be supported: {res:?}"
        );
    }

    #[test]
    fn parse_semver_handles_prefixes_and_prereleases() {
        assert_eq!(parse_semver("0.9.6"), Some((0, 9, 6)));
        assert_eq!(parse_semver("v0.9.6"), Some((0, 9, 6)));
        assert_eq!(parse_semver("V1.2.3-beta.1"), Some((1, 2, 3)));
        assert_eq!(parse_semver("2.0.0+build123"), Some((2, 0, 0)));
        assert_eq!(parse_semver("1.0"), Some((1, 0, 0)));
        assert_eq!(parse_semver("invalid"), None);
    }

    #[test]
    fn compare_versions_stable_logic() {
        assert_eq!(
            compare_versions("0.9.6", "0.9.7", UpdateChannel::Stable),
            VersionComparison::Newer
        );
        assert_eq!(
            compare_versions("0.9.6", "0.9.6", UpdateChannel::Stable),
            VersionComparison::Same
        );
        assert_eq!(
            compare_versions("0.9.7", "0.9.6", UpdateChannel::Stable),
            VersionComparison::Older
        );
        assert_eq!(
            compare_versions("0.9.6-beta", "0.9.6", UpdateChannel::Stable),
            VersionComparison::Newer
        );
        assert_eq!(
            compare_versions("0.9.6", "0.9.6-beta", UpdateChannel::Stable),
            VersionComparison::Older
        );
        assert_eq!(
            compare_versions("1.0.0-rc.1", "1.0.0-rc.2", UpdateChannel::Stable),
            VersionComparison::Newer
        );
        assert_eq!(
            compare_versions("1.0.0-rc.2", "1.0.0", UpdateChannel::Stable),
            VersionComparison::Newer
        );
        assert_eq!(
            compare_versions("1.0.0", "1.0.0-rc.2", UpdateChannel::Stable),
            VersionComparison::Older
        );
        assert_eq!(
            compare_versions("1.0.0-rc.2", "1.0.0-rc.1", UpdateChannel::Stable),
            VersionComparison::Older
        );
    }

    #[test]
    fn compare_versions_nightly_logic() {
        assert_eq!(
            compare_versions("0.9.6", "nightly-2026-04-10", UpdateChannel::Nightly),
            VersionComparison::Newer
        );
        assert_eq!(
            compare_versions(
                "nightly-2026-04-10",
                "nightly-2026-04-10",
                UpdateChannel::Nightly
            ),
            VersionComparison::Same
        );
        assert_eq!(
            compare_versions("0.9.6-45", "0.9.6-46", UpdateChannel::Nightly),
            VersionComparison::Newer
        );
        assert_eq!(
            compare_versions("0.9.6-46", "0.9.6-45", UpdateChannel::Nightly),
            VersionComparison::Older
        );
        assert_eq!(
            compare_versions("0.9.6-45", "0.9.6-45", UpdateChannel::Nightly),
            VersionComparison::Same
        );
        assert_eq!(
            compare_versions(
                "nightly-2026-04-10",
                "nightly-2026-04-11",
                UpdateChannel::Nightly
            ),
            VersionComparison::Newer
        );
        assert_eq!(
            compare_versions(
                "nightly-2026-04-11",
                "nightly-2026-04-10",
                UpdateChannel::Nightly
            ),
            VersionComparison::Older
        );
    }

    #[test]
    fn manifest_serialization_round_trip() {
        let mut platforms = HashMap::new();
        platforms.insert(
            "windows-x86_64".to_string(),
            AppPlatformAsset {
                url: "https://example.com/sona.exe".to_string(),
                signature: "sig123".to_string(),
            },
        );
        let manifest = AppUpdateManifest {
            version: "0.9.7".to_string(),
            notes: Some("Fix bug".to_string()),
            pub_date: Some("2026-04-10T12:00:00Z".to_string()),
            platforms,
        };
        let json = serde_json::to_string(&manifest).unwrap();
        let parsed: AppUpdateManifest = serde_json::from_str(&json).unwrap();
        assert_eq!(manifest, parsed);
    }

    #[test]
    fn channel_parsing_and_display() {
        assert_eq!(
            "stable".parse::<UpdateChannel>().unwrap(),
            UpdateChannel::Stable
        );
        assert_eq!(
            "nightly".parse::<UpdateChannel>().unwrap(),
            UpdateChannel::Nightly
        );
        assert!("invalid".parse::<UpdateChannel>().is_err());
        assert_eq!(UpdateChannel::Stable.to_string(), "stable");
        assert_eq!(UpdateChannel::Nightly.to_string(), "nightly");
    }

    #[test]
    fn release_url_formatting() {
        assert_eq!(
            release_url_for_version("0.9.7", UpdateChannel::Stable),
            "https://github.com/AirSodaz/sona/releases/tag/v0.9.7"
        );
        assert_eq!(
            release_url_for_version("v0.9.7", UpdateChannel::Stable),
            "https://github.com/AirSodaz/sona/releases/tag/v0.9.7"
        );
        assert_eq!(
            release_url_for_version("any", UpdateChannel::Nightly),
            "https://github.com/AirSodaz/sona/releases/tag/nightly"
        );
    }
    #[test]
    fn parse_version_lenient_handles_formats() {
        assert_eq!(
            parse_version_lenient("1.0.0-rc.1"),
            Some(semver::Version::parse("1.0.0-rc.1").unwrap())
        );
        assert_eq!(
            parse_version_lenient("v1.0.0-rc.2"),
            Some(semver::Version::parse("1.0.0-rc.2").unwrap())
        );
        assert_eq!(
            parse_version_lenient("0.8.2-45"),
            Some(semver::Version::parse("0.8.2-45").unwrap())
        );
        assert_eq!(
            parse_version_lenient("1.0"),
            Some(semver::Version::parse("1.0.0").unwrap())
        );
        assert_eq!(
            parse_version_lenient("1.0-beta"),
            Some(semver::Version::parse("1.0.0-beta").unwrap())
        );
        assert_eq!(parse_version_lenient("invalid"), None);
    }

    #[test]
    fn app_update_error_display() {
        let err1 = AppUpdateError::UnsupportedPlatform {
            os: "solaris".to_string(),
            arch: "sparc".to_string(),
        };
        assert_eq!(err1.to_string(), "Unsupported platform: solaris-sparc");
        let err2 = AppUpdateError::UnknownChannel("canary".to_string());
        assert_eq!(
            err2.to_string(),
            "Unknown update channel: 'canary'. Expected 'stable' or 'nightly'"
        );
    }
}
