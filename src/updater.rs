use anyhow::{bail, Context, Result};
use semver::Version;
use sha2::{Digest, Sha256};
use std::io::Write;

const GITHUB_API_LATEST: &str =
    "https://api.github.com/repos/Londopy/HideDesktopApps/releases/latest";
const GITHUB_API_RELEASES: &str = "https://api.github.com/repos/Londopy/HideDesktopApps/releases";

// figure out which arch we're on so we download the right zip
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

// get the latest release info from github
fn fetch_latest_release(channel: &str) -> Result<GithubRelease> {
    let client = build_client()?;

    if channel == "beta" {
        // beta: scan all releases including pre-releases, pick the newest
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
        // stable: just use the latest non-prerelease
        client
            .get(GITHUB_API_LATEST)
            .send()
            .context("Fetching latest release")?
            .json()
            .context("Parsing latest release")
    }
}

// check if there's a newer version available
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

// download the update zip and replace the running exe
pub fn download_and_apply(channel: &str) -> Result<()> {
    let release = fetch_latest_release(channel)?;
    let suffix = arch_suffix();
    let client = build_client()?;

    // Find the zip for our arch. Prefer an arch-specific bare zip
    // (e.g. "...-x64-portable.zip"); otherwise fall back to the combined
    // portable zip ("...-portable.zip", no arch token), which holds
    // x64/x86/arm64 subfolders we extract our arch's exe from below.
    let arch_zip = format!("-{}-portable", suffix);
    let is_combined = |name: &str| {
        name.ends_with("-portable.zip")
            && !["x64", "x86", "arm64"]
                .iter()
                .any(|s| name.contains(&format!("-{}-portable", s)))
    };
    let zip_asset = release
        .assets
        .iter()
        .find(|a| a.name.ends_with(".zip") && a.name.contains(&arch_zip))
        .or_else(|| release.assets.iter().find(|a| is_combined(&a.name)))
        .ok_or_else(|| anyhow::anyhow!("No zip asset found for arch '{}'", suffix))?;

    // find the matching sha256 file if there is one
    let sha_asset = release
        .assets
        .iter()
        .find(|a| a.name == format!("{}.sha256", zip_asset.name));

    // download to the temp folder
    let temp_dir = std::env::temp_dir();
    let zip_path = temp_dir.join("HideDesktopApps-update.zip");
    let new_exe_path = temp_dir.join("HideDesktopApps-new.exe");

    let zip_bytes = client
        .get(&zip_asset.browser_download_url)
        .send()
        .context("Downloading update zip")?
        .bytes()
        .context("Reading update zip bytes")?;

    // verify the download against the hash if we have one
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
    }

    // save the zip to disk
    let mut f = std::fs::File::create(&zip_path).context("Creating temp zip")?;
    f.write_all(&zip_bytes).context("Writing temp zip")?;
    drop(f);

    // pull the exe out of the zip
    let zip_file = std::fs::File::open(&zip_path).context("Opening zip")?;
    let mut archive = zip::ZipArchive::new(zip_file).context("Parsing zip")?;

    // Pick the exe to install. The combined zip stores each arch under its own
    // subfolder (".../x64/HideDesktopApps.exe"), so prefer the entry under our
    // arch's folder; fall back to any .exe for a flat single-arch zip.
    let arch_dir = format!("/{}/", suffix);
    let arch_prefix = format!("{}/", suffix);
    let mut target_idx: Option<usize> = None;
    let mut any_exe_idx: Option<usize> = None;
    for i in 0..archive.len() {
        let file = archive.by_index(i).context("Reading zip entry")?;
        let name = file.name().replace('\\', "/");
        if name.ends_with(".exe") {
            if any_exe_idx.is_none() {
                any_exe_idx = Some(i);
            }
            if name.contains(&arch_dir) || name.starts_with(&arch_prefix) {
                target_idx = Some(i);
                break;
            }
        }
    }

    let idx = target_idx
        .or(any_exe_idx)
        .ok_or_else(|| anyhow::anyhow!("No .exe found inside update zip"))?;

    {
        let mut file = archive.by_index(idx).context("Reading zip entry")?;
        let mut out = std::fs::File::create(&new_exe_path).context("Creating new exe")?;
        std::io::copy(&mut file, &mut out).context("Extracting exe")?;
    }

    // replace the running exe and restart
    self_replace::self_replace(&new_exe_path).context("self_replace failed")?;

    let current_exe = std::env::current_exe().context("Getting current exe path")?;
    std::process::Command::new(current_exe)
        .spawn()
        .context("Spawning new process")?;

    std::process::exit(0);
}

// background update check, sends result to main loop
// user_triggered=true means show a "you're up to date" notification too
pub fn background_check(
    config: crate::config::UpdaterConfig,
    cmd_tx: std::sync::mpsc::Sender<crate::Cmd>,
    user_triggered: bool,
) {
    std::thread::spawn(move || {
        if !config.enabled && !user_triggered {
            return;
        }

        match check_for_update(&config.channel) {
            Ok(Some(version)) => {
                let _ = cmd_tx.send(crate::Cmd::UpdateAvailable(version));
            }
            Ok(None) => {
                if user_triggered {
                    let _ = cmd_tx.send(crate::Cmd::UpToDate);
                }
            }
            Err(e) => {
                eprintln!("Update check failed: {e}");
            }
        }
    });
}

// start the download + apply in a background thread
pub fn background_apply(channel: String) {
    std::thread::spawn(move || {
        if let Err(e) = download_and_apply(&channel) {
            eprintln!("Update failed: {e}");
        }
    });
}
