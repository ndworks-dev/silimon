//! Automatic updates from the latest release published on GitHub.
//!
//! The manifest names the version and the archive's hash. Authenticity comes from the code
//! signature: only an app signed with this project's Developer ID is installed.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const RELEASES_URL: &str = "https://github.com/ndworks-dev/silimon/releases/latest";

const MANIFEST_URL: &str =
    "https://github.com/ndworks-dev/silimon/releases/latest/download/latest.json";
const DOWNLOAD_PREFIX: &str = "https://github.com/ndworks-dev/silimon/releases/download/";
const REQUIREMENT: &str = "anchor apple generic and identifier \"dev.silimon.app\" \
     and certificate leaf[subject.OU] = \"3BRKN94K4H\"";

#[derive(Clone, Debug, Deserialize, PartialEq)]
pub struct Manifest {
    pub version: String,
    pub url: String,
    pub sha256: String,
}

fn parse_version(value: &str) -> Option<[u64; 3]> {
    let mut parts = value.split('.').map(|part| part.parse::<u64>().ok());
    let version = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(version)
}

pub fn is_newer(candidate: &str, current: &str) -> bool {
    matches!((parse_version(candidate), parse_version(current)), (Some(a), Some(b)) if a > b)
}

pub fn parse_manifest(json: &str) -> Result<Manifest, String> {
    let manifest: Manifest =
        serde_json::from_str(json).map_err(|e| format!("invalid manifest: {e}"))?;
    if parse_version(&manifest.version).is_none() {
        return Err("the manifest has an invalid version".into());
    }
    if !manifest.url.starts_with(DOWNLOAD_PREFIX) {
        return Err("the manifest points to a disallowed address".into());
    }
    if manifest.sha256.len() != 64 || !manifest.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("the manifest has an invalid hash".into());
    }
    Ok(manifest)
}

fn run(program: &str, args: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("could not run {program}: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

fn curl(extra: &[&str]) -> Result<String, String> {
    let mut args = vec![
        "--proto", "=https", "--tlsv1.2", "--fail", "--silent", "--show-error", "--location",
        "--connect-timeout", "15",
    ];
    args.extend_from_slice(extra);
    run("/usr/bin/curl", &args)
}

/// Returns the manifest when a version newer than the installed one exists.
pub fn check() -> Result<Option<Manifest>, String> {
    let json = curl(&["--max-time", "30", MANIFEST_URL])?;
    let manifest = parse_manifest(&json)?;
    Ok(is_newer(&manifest.version, VERSION).then_some(manifest))
}

/// Path of the running app, or None when it does not run from an `.app` bundle.
pub fn bundle_path() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?.canonicalize().ok()?;
    let bundle = executable.parent()?.parent()?.parent()?;
    (bundle.extension()? == "app").then(|| bundle.to_path_buf())
}

/// Removes the temporary folder even when installation fails midway.
struct Staging(PathBuf);

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn staging_for(app: &Path) -> Result<Staging, String> {
    let parent = app.parent().ok_or("invalid app path")?;
    let dir = parent.join(format!(".silimon-update-{}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir(&dir).map_err(|e| format!("no permission in {}: {e}", parent.display()))?;
    Ok(Staging(dir))
}

pub fn install(manifest: &Manifest, app: &Path) -> Result<(), String> {
    let staging = staging_for(app)?;
    let archive = staging.0.join("Silimon.zip");
    let archive_path = archive.to_str().ok_or("invalid path")?;
    curl(&["--max-time", "300", "--max-filesize", "52428800", "--output", archive_path, &manifest.url])?;
    apply(&archive, &manifest.sha256, app, &staging)
}

/// Installs an already downloaded archive, with the same checks as an update.
pub fn install_archive(archive: &Path, sha256: &str, app: &Path) -> Result<(), String> {
    let staging = staging_for(app)?;
    apply(archive, sha256, app, &staging)
}

fn apply(archive: &Path, sha256: &str, app: &Path, staging: &Staging) -> Result<(), String> {
    let archive = archive.to_str().ok_or("invalid path")?;
    let digest = run("/usr/bin/shasum", &["-a", "256", archive])?;
    if !digest.split_whitespace().next().is_some_and(|d| d.eq_ignore_ascii_case(sha256)) {
        return Err("the download does not match the published hash".into());
    }

    let unpacked = staging.0.join("unpacked");
    let unpacked_path = unpacked.to_str().ok_or("invalid path")?;
    run("/usr/bin/ditto", &["-x", "-k", archive, unpacked_path])?;
    let new_app = unpacked.join("Silimon.app");
    let new_app_path = new_app.to_str().ok_or("invalid path")?;
    let requirement = format!("-R={REQUIREMENT}");
    run("/usr/bin/codesign", &["--verify", "--deep", "--strict", &requirement, new_app_path])
        .map_err(|e| format!("the app's signature was refused: {e}"))?;

    let previous = staging.0.join("previous.app");
    fs::rename(app, &previous).map_err(|e| format!("could not replace the app: {e}"))?;
    if let Err(error) = fs::rename(&new_app, app) {
        let _ = fs::rename(&previous, app);
        return Err(format!("could not replace the app: {error}"));
    }
    Ok(())
}

/// Reopens the app as soon as this process exits.
pub fn relaunch(app: &Path) -> Result<(), String> {
    Command::new("/bin/sh")
        .args(["-c", "while kill -0 \"$1\" 2>/dev/null; do sleep 0.2; done; exec /usr/bin/open \"$2\""])
        .arg("silimon-relaunch")
        .arg(std::process::id().to_string())
        .arg(app)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|e| format!("could not reopen the app: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const HASH: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    fn manifest(version: &str, url: &str, sha256: &str) -> String {
        format!(r#"{{"version":"{version}","url":"{url}","sha256":"{sha256}"}}"#)
    }

    fn release_url() -> String {
        format!("{DOWNLOAD_PREFIX}v1.2.3/Silimon.zip")
    }

    #[test]
    fn compares_versions_numerically() {
        assert!(is_newer("0.10.0", "0.9.9"));
        assert!(is_newer("1.0.0", "0.99.99"));
        assert!(!is_newer("0.1.0", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
    }

    #[test]
    fn malformed_versions_are_never_newer() {
        assert!(!is_newer("9.9", "0.1.0"));
        assert!(!is_newer("9.9.9-beta", "0.1.0"));
        assert!(!is_newer("9.9.9.9", "0.1.0"));
    }

    #[test]
    fn accepts_a_valid_manifest() {
        let parsed = parse_manifest(&manifest("1.2.3", &release_url(), HASH)).unwrap();
        assert_eq!(parsed.version, "1.2.3");
    }

    #[test]
    fn rejects_downloads_outside_the_project_releases() {
        for url in [
            "https://example.com/Silimon.zip",
            "http://github.com/ndworks-dev/silimon/releases/download/v1/Silimon.zip",
            "https://github.com/someone/silimon/releases/download/v1/Silimon.zip",
        ] {
            assert!(parse_manifest(&manifest("1.2.3", url, HASH)).is_err(), "{url}");
        }
    }

    #[test]
    fn rejects_invalid_hashes_and_versions() {
        assert!(parse_manifest(&manifest("1.2.3", &release_url(), "abc")).is_err());
        assert!(parse_manifest(&manifest("1.2.3", &release_url(), &"z".repeat(64))).is_err());
        assert!(parse_manifest(&manifest("latest", &release_url(), HASH)).is_err());
        assert!(parse_manifest("not json").is_err());
    }
}
