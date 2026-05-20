use anyhow::{bail, Context, Result};
use semver::Version;
use sha2::{Digest, Sha256};
use std::io::Write;

const GITHUB_API_LATEST: &str =
    "https://api.github.com/repos/Londopy/HideDesktopApps/releases/latest";
const GITHUB_API_RELEASES: &str = "https://api.github.com/repos/Londopy/HideDesktopApps/releases";

/// Detect the current build architecture suffix used in release asset names.
fn arch_suffix() -> &'static str {
    #[cfg(target_arch = "x86_64")]
    {
        "x64"
    }
    #[cfg(target_arch = "x86")]
    {
        "x86"
    }
    #[cfg(target_arch = "aarch64")]
    {
        "arm64"
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "x86", target_arch = "aarch64")))]
    {
        "x64"
    }
}

#[derive(Debug, serde::Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Debug, serde::Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

fn build_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(format!("HideDesktopApps/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .context("Building HTTP client")
}

/// Fetch the latest release from GitHub. For beta channel, scans all releases.
fn fetch_latest_release(channel: &str) -> Result<GithubRelease> {
    let client = build_client()?;

    if channel == "beta" {
        // Scan all releases (includes pre-releases) and pick the newest
        let releases: Vec<GithubRelease> = client
            .get(GITHUB_API_RELEASES)
            .send()
            .context("Fetching releases list")?
            .json()
            .context("Parsing releases list")?;

        releases
            .into_iter()
            .max_by(|a, b| {
                let va = Version::parse(a.tag_name.trim_start_matches('v')).ok();
                let vb = Version::parse(b.tag_name.trim_start_matches('v')).ok();
                va.cmp(&vb)
            })
            .ok_or_else(|| anyhow::anyhow!("No releases found"))
    } else {
        // Stable channel: latest non-prerelease
        client
            .get(GITHUB_API_LATEST)
            .send()
            .context("Fetching latest release")?
            .json()
            .context("Parsing latest release")
    }
}

/// Check whether a newer version is available. Returns Some(version_string) if so.
pub fn check_for_update(channel: &str) -> Result<Option<String>> {
    let release = fetch_latest_release(channel)?;
    let remote_str = release.tag_name.trim_start_matches('v');
    let remote = Version::parse(remote_str)
        .with_context(|| format!("Parsing remote version '{}'", remote_str))?;
    let current = Version::parse(env!("CARGO_PKG_VERSION")).context("Parsing current version")?;

    if remote > current {
        Ok(Some(remote_str.to_string()))
    } else {
        Ok(None)
    }
}

/// Download and apply an update, then restart.
pub fn download_and_apply(channel: &str) -> Result<()> {
    let release = fetch_latest_release(channel)?;
    let suffix = arch_suffix();
    let client = build_client()?;

    // Find the portable zip asset
    let zip_asset = release
        .assets
        .iter()
        .find(|a| a.name.contains(suffix) && a.name.ends_with(".zip"))
        .ok_or_else(|| anyhow::anyhow!("No zip asset found for arch '{}'", suffix))?;

    // Find the matching .sha256 asset
    let sha_asset = release
        .assets
        .iter()
        .find(|a| a.name == format!("{}.sha256", zip_asset.name));

    // Download zip to temp
    let temp_dir = std::env::temp_dir();
    let zip_path = temp_dir.join("HideDesktopApps-update.zip");
    let new_exe_path = temp_dir.join("HideDesktopApps-new.exe");

    eprintln!("Downloading update: {}", zip_asset.browser_download_url);
    let zip_bytes = client
        .get(&zip_asset.browser_download_url)
        .send()
        .context("Downloading update zip")?
        .bytes()
        .context("Reading update zip bytes")?;

    // Verify SHA-256 if available
    if let Some(sha_asset) = sha_asset {
        let expected_hex = client
            .get(&sha_asset.browser_download_url)
            .send()
            .context("Downloading sha256")?
            .text()
            .context("Reading sha256 text")?;
        let expected_hex = expected_hex
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_lowercase();

        let mut hasher = Sha256::new();
        hasher.update(&zip_bytes);
        let actual_hex = hex::encode(hasher.finalize());

        if actual_hex != expected_hex {
            bail!(
                "SHA-256 mismatch! expected={}, got={}",
                expected_hex,
                actual_hex
            );
        }
        eprintln!("SHA-256 verified OK");
    } else {
        eprintln!("No .sha256 asset found; skipping verification");
    }

    // Write zip to disk
    let mut f = std::fs::File::create(&zip_path).context("Creating temp zip")?;
    f.write_all(&zip_bytes).context("Writing temp zip")?;
    drop(f);

    // Extract the exe from the zip
    let zip_file = std::fs::File::open(&zip_path).context("Opening zip")?;
    let mut archive = zip::ZipArchive::new(zip_file).context("Parsing zip")?;

    let mut found = false;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Reading zip entry")?;
        if file.name().ends_with(".exe") {
            let mut out = std::fs::File::create(&new_exe_path).context("Creating new exe")?;
            std::io::copy(&mut file, &mut out).context("Extracting exe")?;
            found = true;
            break;
        }
    }

    if !found {
        bail!("No .exe found inside update zip");
    }

    // Self-replace and restart
    eprintln!("Replacing current exe with new version...");
    self_replace::self_replace(&new_exe_path).context("self_replace failed")?;

    let current_exe = std::env::current_exe().context("Getting current exe path")?;
    std::process::Command::new(current_exe)
        .spawn()
        .context("Spawning new process")?;

    std::process::exit(0);
}

/// Run a background update check and send the result to the main loop.
pub fn background_check(
    config: crate::config::UpdaterConfig,
    cmd_tx: std::sync::mpsc::Sender<crate::Cmd>,
) {
    std::thread::spawn(move || {
        if !config.enabled {
            return;
        }

        match check_for_update(&config.channel) {
            Ok(Some(version)) => {
                eprintln!("Update available: {version}");
                let _ = cmd_tx.send(crate::Cmd::UpdateAvailable(version));
            }
            Ok(None) => {
                eprintln!("No update available");
            }
            Err(e) => {
                eprintln!("Update check failed: {e}");
            }
        }
    });
}

/// Run a background update download+apply (triggered by user from Settings).
pub fn background_apply(channel: String) {
    std::thread::spawn(move || {
        if let Err(e) = download_and_apply(&channel) {
            eprintln!("Update failed: {e}");
        }
    });
}
