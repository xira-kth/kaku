use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use semver::Version;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_API: &str = "https://api.github.com/repos/xira-kth/kaku/releases/latest";
const CACHE_TTL_SECS: u64 = 60 * 60 * 24;
const NPM_PACKAGE: &str = "@voidique/kaku";

pub fn spawn_update_checker() -> Receiver<Option<String>> {
    let (sender, receiver) = mpsc::channel();

    thread::spawn(move || {
        if let Some(cached) = cached_newer_version() {
            let _ = sender.send(Some(cached));
        }

        if !cache_is_stale() {
            return;
        }

        let latest = fetch_latest_version().ok();
        if let Some(version) = latest.as_deref() {
            let _ = save_cache(version);
        }

        if let Some(version) = latest.filter(|version| is_newer_than_current(version)) {
            let _ = sender.send(Some(version));
        }
    });

    receiver
}

pub fn update_notice(version: &str) -> String {
    format!("Update available: {version}  run: kaku update")
}

pub fn run_update() -> Result<(), String> {
    let latest = fetch_latest_version().unwrap_or_else(|_| CURRENT_VERSION.to_string());
    if !is_newer_than_current(&latest) {
        println!("kaku is up to date ({CURRENT_VERSION})");
        return Ok(());
    }

    let install_source = detect_install_source();
    let mut command = match install_source {
        InstallSource::Homebrew => {
            let mut command = Command::new("brew");
            command.args(["upgrade", "kaku"]);
            command
        }
        InstallSource::Npm => {
            let mut command = Command::new("npm");
            command.args(["install", "-g", &format!("{NPM_PACKAGE}@latest")]);
            command
        }
        InstallSource::StandaloneUnix => {
            let mut command = Command::new("sh");
            command.arg("-c").arg(
                "curl --proto '=https' --tlsv1.2 -LsSf https://github.com/xira-kth/kaku/releases/latest/download/kaku-installer.sh | sh",
            );
            command
        }
        InstallSource::StandaloneWindows => {
            let mut command = Command::new("powershell");
            command.args([
                "-ExecutionPolicy",
                "Bypass",
                "-c",
                "irm https://github.com/xira-kth/kaku/releases/latest/download/kaku-installer.ps1 | iex",
            ]);
            command
        }
    };

    command.stdin(Stdio::inherit());
    command.stdout(Stdio::inherit());
    command.stderr(Stdio::inherit());

    let status = command.status().map_err(|error| error.to_string())?;
    if !status.success() {
        return Err("update command failed".to_string());
    }

    Ok(())
}

fn detect_install_source() -> InstallSource {
    if cfg!(windows) {
        if npm_global_install_exists() {
            InstallSource::Npm
        } else {
            InstallSource::StandaloneWindows
        }
    } else if homebrew_install_exists() {
        InstallSource::Homebrew
    } else if npm_global_install_exists() {
        InstallSource::Npm
    } else {
        InstallSource::StandaloneUnix
    }
}

fn homebrew_install_exists() -> bool {
    Command::new("brew")
        .args(["list", "--versions", "kaku"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn npm_global_install_exists() -> bool {
    Command::new("npm")
        .args(["ls", "-g", NPM_PACKAGE, "--depth=0"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn cached_newer_version() -> Option<String> {
    let contents = fs::read_to_string(cache_file()).ok()?;
    let mut lines = contents.lines();
    let checked = lines.next()?.parse::<u64>().ok()?;
    let version = lines.next()?.to_string();

    if current_unix_time().saturating_sub(checked) > CACHE_TTL_SECS {
        return None;
    }

    is_newer_than_current(&version).then_some(version)
}

fn cache_is_stale() -> bool {
    let Ok(contents) = fs::read_to_string(cache_file()) else {
        return true;
    };

    let Some(first_line) = contents.lines().next() else {
        return true;
    };

    let Ok(checked) = first_line.parse::<u64>() else {
        return true;
    };

    current_unix_time().saturating_sub(checked) > CACHE_TTL_SECS
}

fn save_cache(version: &str) -> Result<(), String> {
    let cache_path = cache_file();
    if let Some(parent) = cache_path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }

    fs::write(cache_path, format!("{}\n{version}\n", current_unix_time()))
        .map_err(|error| error.to_string())
}

fn cache_file() -> PathBuf {
    cache_dir().join("kaku").join("update-check")
}

fn cache_dir() -> PathBuf {
    if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join("Library").join("Caches"))
            .unwrap_or_else(|| PathBuf::from("."))
    } else if let Some(path) = std::env::var_os("XDG_CACHE_HOME").map(PathBuf::from) {
        path
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .map(|path| path.join(".cache"))
            .unwrap_or_else(|| PathBuf::from("."))
    }
}

fn fetch_latest_version() -> Result<String, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(2))
        .build();

    let response = agent
        .get(RELEASES_API)
        .set("User-Agent", &format!("kaku/{CURRENT_VERSION}"))
        .call()
        .map_err(|error: ureq::Error| error.to_string())?;

    let body = response
        .into_string()
        .map_err(|error: std::io::Error| error.to_string())?;
    let tag =
        extract_json_value(&body, "tag_name").ok_or_else(|| "missing tag_name".to_string())?;
    Ok(tag.trim_start_matches('v').to_string())
}

fn extract_json_value(body: &str, key: &str) -> Option<String> {
    let needle = format!("\"{key}\":");
    let start = body.find(&needle)? + needle.len();
    let rest = body.get(start..)?.trim_start();
    if !rest.starts_with('"') {
        return None;
    }

    let mut escaped = false;
    let mut value = String::new();
    for ch in rest[1..].chars() {
        if escaped {
            value.push(ch);
            escaped = false;
            continue;
        }

        match ch {
            '\\' => escaped = true,
            '"' => break,
            _ => value.push(ch),
        }
    }

    Some(value)
}

fn is_newer_than_current(version: &str) -> bool {
    let Ok(current) = Version::parse(CURRENT_VERSION) else {
        return false;
    };
    let Ok(candidate) = Version::parse(version) else {
        return false;
    };
    candidate > current
}

fn current_unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

enum InstallSource {
    Homebrew,
    Npm,
    StandaloneUnix,
    StandaloneWindows,
}

#[cfg(test)]
mod tests {
    use super::{extract_json_value, is_newer_than_current};

    #[test]
    fn extracts_tag_name() {
        let json = r#"{"tag_name":"v0.1.2","name":"release"}"#;
        assert_eq!(
            extract_json_value(json, "tag_name").as_deref(),
            Some("v0.1.2")
        );
    }

    #[test]
    fn compares_versions() {
        assert!(is_newer_than_current("9.9.9"));
        assert!(!is_newer_than_current("0.1.2"));
    }
}
