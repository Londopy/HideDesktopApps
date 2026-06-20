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

// Choose the portable zip asset for `suffix`. Prefers an arch-specific bare zip
// ("...-x64-portable.zip"); falls back to the combined portable zip
// ("...-portable.zip" with no arch token) that holds x64/x86/arm64 subfolders.
fn select_update_zip<'a>(asset_names: &'a [String], suffix: &str) -> Option<&'a String> {
    let arch_zip = format!("-{}-portable", suffix);
    if let Some(n) = asset_names
        .iter()
        .find(|n| n.ends_with(".zip") && n.contains(&arch_zip))
    {
        return Some(n);
    }
    asset_names.iter().find(|n| is_combined_zip(n))
}

// True for the combined portable zip: ends with "-portable.zip" and carries no
// arch token (so "...-x64-portable.zip" etc. are excluded).
fn is_combined_zip(name: &str) -> bool {
    name.ends_with("-portable.zip")
        && !["x64", "x86", "arm64"]
            .iter()
            .any(|s| name.contains(&format!("-{}-portable", s)))
}

// Index of the exe to install: prefer the entry under this arch's subfolder
// (combined zip: ".../x64/HideDesktopApps.exe"); else the first exe (flat zip).
fn pick_arch_exe_index(entry_names: &[String], suffix: &str) -> Option<usize> {
    let arch_dir = format!("/{}/", suffix);
    let arch_prefix = format!("{}/", suffix);
    let mut any_exe: Option<usize> = None;
    for (i, raw) in entry_names.iter().enumerate() {
        let name = raw.replace('\\', "/");
        if name.ends_with(".exe") {
            if any_exe.is_none() {
                any_exe = Some(i);
            }
            if name.contains(&arch_dir) || name.starts_with(&arch_prefix) {
                return Some(i);
            }
        }
    }
    any_exe
}

// Parse a SHA256SUMS.txt body ("<hex>  <filename>" lines) for `filename`'s hash.
fn hash_from_sums(sums: &str, filename: &str) -> Option<String> {
    for line in sums.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let mut parts = line.split_whitespace();
        let hash = match parts.next() {
            Some(h) => h,
            None => continue,
        };
        // sha256sum may prefix the name with '*'; also tolerate path components.
        let name = parts.next().unwrap_or("").trim_start_matches('*');
        let base = name.rsplit(['/', '\\']).next().unwrap_or(name);
        if base == filename {
            return Some(hash.to_lowercase());
        }
    }
    None
}

// Fetch the expected SHA-256 for `zip_name`: a per-file "<zip>.sha256" sidecar
// first, else the consolidated SHA256SUMS.txt. Returns None if neither exists.
fn resolve_expected_hash(
    client: &reqwest::blocking::Client,
    assets: &[GithubAsset],
    zip_name: &str,
) -> Option<String> {
    let sidecar = format!("{}.sha256", zip_name);
    if let Some(a) = assets.iter().find(|a| a.name == sidecar) {
        if let Ok(text) = client
            .get(&a.browser_download_url)
            .send()
            .and_then(|r| r.text())
        {
            if let Some(h) = text.split_whitespace().next() {
                return Some(h.to_lowercase());
            }
        }
    }
    if let Some(a) = assets.iter().find(|a| a.name == "SHA256SUMS.txt") {
        if let Ok(text) = client
            .get(&a.browser_download_url)
            .send()
            .and_then(|r| r.text())
        {
            return hash_from_sums(&text, zip_name);
        }
    }
    None
}

// download the update zip and replace the running exe
pub fn download_and_apply(channel: &str) -> Result<()> {
    let release = fetch_latest_release(channel)?;
    let suffix = arch_suffix();
    let client = build_client()?;

    // Find the zip for our arch (rules live in select_update_zip).
    let asset_names: Vec<String> = release.assets.iter().map(|a| a.name.clone()).collect();
    let zip_name = select_update_zip(&asset_names, suffix)
        .ok_or_else(|| anyhow::anyhow!("No zip asset found for arch '{}'", suffix))?;
    let zip_asset = release
        .assets
        .iter()
        .find(|a| &a.name == zip_name)
        .expect("selected name came from the asset list");

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

    // Verify against the expected hash: per-file ".sha256" if present, else the
    // consolidated SHA256SUMS.txt. If neither exists, log and proceed unverified.
    match resolve_expected_hash(&client, &release.assets, &zip_asset.name) {
        Some(expected_hex) => {
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
        None => crate::dlog!("updater: no checksum asset found, skipping verification"),
    }

    // save the zip to disk
    let mut f = std::fs::File::create(&zip_path).context("Creating temp zip")?;
    f.write_all(&zip_bytes).context("Writing temp zip")?;
    drop(f);

    // pull the exe out of the zip
    let zip_file = std::fs::File::open(&zip_path).context("Opening zip")?;
    let mut archive = zip::ZipArchive::new(zip_file).context("Parsing zip")?;

    // Pick the exe under our arch's subfolder (combined zip) or the lone exe
    // (flat zip). Rule lives in pick_arch_exe_index.
    let entry_names: Vec<String> = (0..archive.len())
        .map(|i| archive.by_index(i).map(|f| f.name().to_string()))
        .collect::<std::result::Result<Vec<String>, _>>()
        .context("Reading zip entries")?;
    let idx = pick_arch_exe_index(&entry_names, suffix)
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
                crate::dlog!("Update check failed: {e}");
                eprintln!("Update check failed: {e}");
            }
        }
    });
}

// start the download + apply in a background thread
pub fn background_apply(channel: String) {
    std::thread::spawn(move || {
        if let Err(e) = download_and_apply(&channel) {
            crate::dlog!("Update failed: {e}");
            eprintln!("Update failed: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    // Mirror of installer.iss VersionInfoVersion derivation, kept here so CI
    // regression-tests the "strip prerelease suffix" rule.
    fn numeric_version(v: &str) -> &str {
        v.split('-').next().unwrap_or(v)
    }

    #[test]
    fn numeric_version_strips_prerelease() {
        assert_eq!(numeric_version("1.1.4-rc1"), "1.1.4");
        assert_eq!(numeric_version("2.3.4-beta.2"), "2.3.4");
        assert_eq!(numeric_version("1.1.4"), "1.1.4");
        assert_eq!(numeric_version("10.20.30-rc.5"), "10.20.30");
    }

    #[test]
    fn select_zip_prefers_arch_specific_then_combined() {
        let assets = names(&[
            "HideDesktopApps-v1.2.0-x64-setup.exe",
            "HideDesktopApps-v1.2.0-portable.zip",
            "HideDesktopApps-v1.2.0-x64-portable.zip",
            "SHA256SUMS.txt",
        ]);
        assert_eq!(
            select_update_zip(&assets, "x64").map(String::as_str),
            Some("HideDesktopApps-v1.2.0-x64-portable.zip")
        );
        assert_eq!(
            select_update_zip(&assets, "x86").map(String::as_str),
            Some("HideDesktopApps-v1.2.0-portable.zip")
        );
        assert_eq!(
            select_update_zip(&assets, "arm64").map(String::as_str),
            Some("HideDesktopApps-v1.2.0-portable.zip")
        );
    }

    #[test]
    fn select_zip_combined_only_resolves_every_arch() {
        let assets = names(&["HideDesktopApps-v2.0.0-portable.zip", "SHA256SUMS.txt"]);
        for arch in ["x64", "x86", "arm64"] {
            assert_eq!(
                select_update_zip(&assets, arch).map(String::as_str),
                Some("HideDesktopApps-v2.0.0-portable.zip")
            );
        }
    }

    #[test]
    fn select_zip_legacy_per_arch_still_works() {
        let assets = names(&[
            "HideDesktopApps-v1.0.0-x64-portable.zip",
            "HideDesktopApps-v1.0.0-x86-portable.zip",
            "HideDesktopApps-v1.0.0-arm64-portable.zip",
        ]);
        assert_eq!(
            select_update_zip(&assets, "arm64").map(String::as_str),
            Some("HideDesktopApps-v1.0.0-arm64-portable.zip")
        );
    }

    #[test]
    fn pick_exe_from_combined_subfolders() {
        let entries = names(&[
            "HideDesktopApps/x64/HideDesktopApps.exe",
            "HideDesktopApps/x86/HideDesktopApps.exe",
            "HideDesktopApps/arm64/HideDesktopApps.exe",
        ]);
        assert_eq!(pick_arch_exe_index(&entries, "x86"), Some(1));
        assert_eq!(pick_arch_exe_index(&entries, "arm64"), Some(2));
        assert_eq!(pick_arch_exe_index(&entries, "x64"), Some(0));
    }

    #[test]
    fn pick_exe_flat_zip_falls_back_to_only_exe() {
        let entries = names(&["HideDesktopApps.exe"]);
        assert_eq!(pick_arch_exe_index(&entries, "x86"), Some(0));
    }

    #[test]
    fn pick_exe_handles_backslash_paths() {
        let entries = names(&["HideDesktopApps\\arm64\\HideDesktopApps.exe"]);
        assert_eq!(pick_arch_exe_index(&entries, "arm64"), Some(0));
    }

    #[test]
    fn hash_from_sums_matches_filename() {
        let sums = "\
aaaa1111  HideDesktopApps-v1.2.0-x64-setup.exe
bbbb2222  HideDesktopApps-v1.2.0-portable.zip
cccc3333  SHA256SUMS-decoy.zip
";
        assert_eq!(
            hash_from_sums(sums, "HideDesktopApps-v1.2.0-portable.zip").as_deref(),
            Some("bbbb2222")
        );
        assert_eq!(hash_from_sums(sums, "not-present.zip"), None);
    }

    #[test]
    fn hash_from_sums_tolerates_star_and_blank_lines() {
        let sums = "\n  DEAFBEEF  *file.zip\n\n";
        assert_eq!(
            hash_from_sums(sums, "file.zip").as_deref(),
            Some("deafbeef")
        );
    }
}
