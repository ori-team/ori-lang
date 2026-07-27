// Self-update from GitHub Releases: `ori update [--check]` (LANG-CLI-1).
//
// Release discovery uses the list endpoint (`/releases?per_page=1`) because
// every Ori release is published as a prerelease, which `/releases/latest`
// refuses to serve (it 404s). Archive integrity comes from the sha256
// `digest` GitHub attaches to each asset, so the checksum travels in the
// same response that names the download URL — no extra checksum assets.
//
// The swap stages everything inside the install root so every move is a
// same-filesystem rename: fresh files land in `.ori-update-staging/`,
// replaced entries are parked in `.ori-update-backup/` until the swap
// finishes (enabling rollback), and leftovers from a crashed or
// Windows-locked run are swept on the next update.

use std::path::{Path, PathBuf};

use crate::pipeline;

const DEFAULT_RELEASES_API_URL: &str =
    "https://api.github.com/repos/raillen/ori-lang/releases?per_page=1";
/// Test/mirror override for the release listing endpoint.
const RELEASES_API_URL_ENV: &str = "ORI_UPDATE_RELEASES_URL";
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const HTTP_USER_AGENT: &str = concat!("ori-cli/", env!("CARGO_PKG_VERSION"));
const STAGING_DIR_NAME: &str = ".ori-update-staging";
const BACKUP_DIR_NAME: &str = ".ori-update-backup";

pub struct UpdateOptions {
    /// Report whether a newer release exists without installing it.
    pub check_only: bool,
    /// Overrides `std::env::current_exe()`; lets tests update a fake install.
    pub exe_path: Option<PathBuf>,
}

#[derive(Debug)]
pub enum UpdateOutcome {
    UpToDate {
        current: String,
    },
    UpdateAvailable {
        current: String,
        latest: String,
    },
    Installed {
        previous: String,
        installed: String,
        install_root: PathBuf,
    },
}

pub fn run_update(options: &UpdateOptions) -> Result<UpdateOutcome, String> {
    let release = fetch_newest_release()?;
    if !version_is_newer(&release.version, CURRENT_VERSION) {
        return Ok(UpdateOutcome::UpToDate {
            current: CURRENT_VERSION.to_string(),
        });
    }
    if options.check_only {
        return Ok(UpdateOutcome::UpdateAvailable {
            current: CURRENT_VERSION.to_string(),
            latest: release.version,
        });
    }

    let exe_path = match &options.exe_path {
        Some(path) => path.clone(),
        None => std::env::current_exe()
            .map_err(|e| format!("update.install_failed: cannot locate the running binary: {e}"))?,
    };
    let install = locate_packaged_install(&exe_path)?;
    refuse_system_package_install(&install.root)?;

    let target = pipeline::native_target_triple();
    let archive_name = release_archive_name(&release.version, &target);
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == archive_name)
        .ok_or_else(|| {
            format!(
                "update.asset_missing: release v{} has no `{archive_name}` package for target `{target}`",
                release.version
            )
        })?;
    install_release_asset(&install, asset)?;

    Ok(UpdateOutcome::Installed {
        previous: CURRENT_VERSION.to_string(),
        installed: release.version,
        install_root: install.root,
    })
}

// ─── Release discovery ───────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct ReleaseEntry {
    tag_name: String,
    #[serde(default)]
    assets: Vec<ReleaseAsset>,
}

#[derive(serde::Deserialize)]
struct ReleaseAsset {
    name: String,
    browser_download_url: String,
    /// GitHub-computed checksum, e.g. `sha256:<hex>`.
    #[serde(default)]
    digest: Option<String>,
}

struct NewestRelease {
    /// Version without the `v` tag prefix, e.g. `0.3.7`.
    version: String,
    assets: Vec<ReleaseAsset>,
}

fn releases_api_url() -> String {
    std::env::var(RELEASES_API_URL_ENV)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_RELEASES_API_URL.to_string())
}

fn fetch_newest_release() -> Result<NewestRelease, String> {
    let url = releases_api_url();
    let response = ureq::get(&url)
        .set("User-Agent", HTTP_USER_AGENT)
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| format!("update.release_lookup_failed: GET {url}: {e}"))?;
    let entries: Vec<ReleaseEntry> = response
        .into_json()
        .map_err(|e| format!("update.release_lookup_failed: invalid JSON from {url}: {e}"))?;
    let newest = entries
        .into_iter()
        .next()
        .ok_or_else(|| format!("update.release_lookup_failed: {url} returned no releases"))?;
    let version = newest.tag_name.trim_start_matches('v').to_string();
    if parse_version_triple(&version).is_none() {
        return Err(format!(
            "update.release_lookup_failed: unrecognized release tag `{}`",
            newest.tag_name
        ));
    }
    Ok(NewestRelease {
        version,
        assets: newest.assets,
    })
}

fn parse_version_triple(version: &str) -> Option<(u64, u64, u64)> {
    let version = version.strip_prefix('v').unwrap_or(version);
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    let patch = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some((major, minor, patch))
}

fn version_is_newer(candidate: &str, current: &str) -> bool {
    match (
        parse_version_triple(candidate),
        parse_version_triple(current),
    ) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

/// Mirrors the asset naming in `release.yml`: `.zip` for the MSVC package,
/// `.tar.gz` everywhere else.
fn release_archive_name(version: &str, target: &str) -> String {
    let extension = if target.contains("windows-msvc") {
        "zip"
    } else {
        "tar.gz"
    };
    format!("ori-v{version}-{target}.{extension}")
}

// ─── Install layout guards ───────────────────────────────────────────────────

#[derive(Debug)]
struct PackagedInstall {
    root: PathBuf,
}

/// A packaged install keeps `runtime/` beside the binary (flat package
/// layout) or beside the binary's parent (`bin/` layout). Development builds
/// have neither and must update via git + cargo instead.
fn locate_packaged_install(exe_path: &Path) -> Result<PackagedInstall, String> {
    let exe_dir = exe_path.parent().ok_or_else(|| {
        format!(
            "update.unpackaged_install: `{}` has no parent directory",
            exe_path.display()
        )
    })?;
    for root in [Some(exe_dir), exe_dir.parent()].into_iter().flatten() {
        if root.join("runtime").is_dir() {
            return Ok(PackagedInstall {
                root: root.to_path_buf(),
            });
        }
    }
    Err(format!(
        "update.unpackaged_install: no `runtime/` directory next to `{}`; this looks like a development build — update via git + cargo instead",
        exe_path.display()
    ))
}

/// `.deb`-style installs are owned by the system package manager; replacing
/// their files desyncs dpkg. `/usr/local` stays allowed (manual installs).
fn refuse_system_package_install(root: &Path) -> Result<(), String> {
    if root.starts_with("/usr") && !root.starts_with("/usr/local") {
        return Err(format!(
            "update.system_package_install: `{}` is managed by the system package manager; install the new release package (e.g. the .deb) instead",
            root.display()
        ));
    }
    Ok(())
}

// ─── Download, verify, swap ──────────────────────────────────────────────────

fn install_release_asset(install: &PackagedInstall, asset: &ReleaseAsset) -> Result<(), String> {
    let expected_sha256 = expected_sha256(asset)?;
    let staging_dir = install.root.join(STAGING_DIR_NAME);
    let backup_dir = install.root.join(BACKUP_DIR_NAME);
    remove_leftover_dir(&staging_dir);
    remove_leftover_dir(&backup_dir);
    std::fs::create_dir_all(&staging_dir).map_err(|e| {
        format!(
            "update.install_failed: create staging dir {}: {e}",
            staging_dir.display()
        )
    })?;

    let result: Result<(), String> = (|| {
        let archive = staging_dir.join(&asset.name);
        download_asset(asset, &archive)?;
        verify_archive_checksum(&archive, &expected_sha256)?;
        let extract_dir = staging_dir.join("extracted");
        std::fs::create_dir_all(&extract_dir).map_err(|e| {
            format!(
                "update.install_failed: create {}: {e}",
                extract_dir.display()
            )
        })?;
        extract_archive(&archive, &extract_dir)?;
        let package_root = find_extracted_package_root(&extract_dir)?;
        swap_package_entries(install, &package_root, &backup_dir)
    })();

    // On Windows the parked running exe cannot be deleted while it lives;
    // leftovers are swept at the start of the next update.
    remove_leftover_dir(&staging_dir);
    if result.is_ok() {
        remove_leftover_dir(&backup_dir);
    }
    result
}

fn expected_sha256(asset: &ReleaseAsset) -> Result<String, String> {
    let digest = asset.digest.as_deref().ok_or_else(|| {
        format!(
            "update.checksum_missing: release asset `{}` has no sha256 digest; refusing to install unverified bytes",
            asset.name
        )
    })?;
    digest
        .strip_prefix("sha256:")
        .map(str::to_string)
        .ok_or_else(|| {
            format!(
                "update.checksum_missing: unrecognized digest format `{digest}` for `{}`",
                asset.name
            )
        })
}

fn download_asset(asset: &ReleaseAsset, dest: &Path) -> Result<(), String> {
    let response = ureq::get(&asset.browser_download_url)
        .set("User-Agent", HTTP_USER_AGENT)
        .call()
        .map_err(|e| {
            format!(
                "update.download_failed: GET {}: {e}",
                asset.browser_download_url
            )
        })?;
    let mut reader = response.into_reader();
    let mut file = std::fs::File::create(dest)
        .map_err(|e| format!("update.download_failed: create {}: {e}", dest.display()))?;
    std::io::copy(&mut reader, &mut file)
        .map_err(|e| format!("update.download_failed: write {}: {e}", dest.display()))?;
    Ok(())
}

pub fn sha256_hex_of_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("update.checksum_failed: open {}: {e}", path.display()))?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)
        .map_err(|e| format!("update.checksum_failed: read {}: {e}", path.display()))?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn verify_archive_checksum(archive: &Path, expected_hex: &str) -> Result<(), String> {
    let actual = sha256_hex_of_file(archive)?;
    if !actual.eq_ignore_ascii_case(expected_hex) {
        return Err(format!(
            "update.checksum_mismatch: `{}` hashed to {actual} but the release manifest says {expected_hex}; aborting",
            archive.display()
        ));
    }
    Ok(())
}

/// Uses the system `tar`: GNU tar and bsdtar auto-detect gzip with `-xf`,
/// and Windows 10+ ships bsdtar in System32, which also extracts `.zip`.
fn extract_archive(archive: &Path, dest: &Path) -> Result<(), String> {
    let output = std::process::Command::new("tar")
        .arg("-xf")
        .arg(archive)
        .arg("-C")
        .arg(dest)
        .output()
        .map_err(|e| format!("update.extract_failed: could not run `tar`: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "update.extract_failed: tar -xf {}: {}",
            archive.display(),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

/// Release archives hold exactly one top-level `ori-<target>/` directory.
fn find_extracted_package_root(extract_dir: &Path) -> Result<PathBuf, String> {
    let mut directories = Vec::new();
    let entries = std::fs::read_dir(extract_dir)
        .map_err(|e| format!("update.extract_failed: read {}: {e}", extract_dir.display()))?;
    for entry in entries.flatten() {
        if entry.path().is_dir() {
            directories.push(entry.path());
        }
    }
    match directories.as_slice() {
        [single] => Ok(single.clone()),
        _ => Err(format!(
            "update.extract_failed: expected one package directory inside the archive, found {} in {}",
            directories.len(),
            extract_dir.display()
        )),
    }
}

/// Replaces each top-level package entry (`ori`, `ori-lsp`, `runtime/`,
/// `stdlib/`, `examples/`, …) in the install root with the freshly extracted
/// one, parking the old entry in `backup_dir` so a mid-swap failure can roll
/// everything back.
fn swap_package_entries(
    install: &PackagedInstall,
    package_root: &Path,
    backup_dir: &Path,
) -> Result<(), String> {
    std::fs::create_dir_all(backup_dir).map_err(|e| {
        format!(
            "update.swap_failed: create backup dir {}: {e}",
            backup_dir.display()
        )
    })?;
    let entries = std::fs::read_dir(package_root)
        .map_err(|e| format!("update.swap_failed: read {}: {e}", package_root.display()))?;
    let mut swapped: Vec<(PathBuf, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let fresh = entry.path();
        let live = install.root.join(entry.file_name());
        let parked = backup_dir.join(entry.file_name());
        if live.exists() {
            if let Err(e) = std::fs::rename(&live, &parked) {
                restore_swapped_entries(&swapped);
                return Err(format!("update.swap_failed: park {}: {e}", live.display()));
            }
            swapped.push((parked.clone(), live.clone()));
        }
        if let Err(e) = std::fs::rename(&fresh, &live) {
            restore_swapped_entries(&swapped);
            return Err(format!("update.swap_failed: place {}: {e}", live.display()));
        }
    }
    Ok(())
}

fn restore_swapped_entries(swapped: &[(PathBuf, PathBuf)]) {
    for (parked, live) in swapped.iter().rev() {
        let _ = std::fs::remove_dir_all(live);
        let _ = std::fs::remove_file(live);
        let _ = std::fs::rename(parked, live);
    }
}

fn remove_leftover_dir(path: &Path) {
    let _ = std::fs::remove_dir_all(path);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_DIR_ID: AtomicU64 = AtomicU64::new(0);

    fn unique_temp_dir(name: &str) -> PathBuf {
        let id = NEXT_DIR_ID.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "ori_update_unit_{}_{}_{}",
            std::process::id(),
            id,
            name
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn version_ordering_uses_numeric_triples() {
        assert!(version_is_newer("0.3.7", "0.3.6"));
        assert!(version_is_newer("0.4.0", "0.3.9"));
        assert!(version_is_newer("1.0.0", "0.9.9"));
        assert!(version_is_newer("v0.3.10", "0.3.9"));
        assert!(!version_is_newer("0.3.6", "0.3.6"));
        assert!(!version_is_newer("0.3.5", "0.3.6"));
        assert!(!version_is_newer("not-a-version", "0.3.6"));
        assert!(!version_is_newer("0.3", "0.3.6"));
    }

    #[test]
    fn release_archive_names_match_packaging_matrix() {
        assert_eq!(
            release_archive_name("0.3.7", "x86_64-unknown-linux-gnu"),
            "ori-v0.3.7-x86_64-unknown-linux-gnu.tar.gz"
        );
        assert_eq!(
            release_archive_name("0.3.7", "aarch64-apple-darwin"),
            "ori-v0.3.7-aarch64-apple-darwin.tar.gz"
        );
        assert_eq!(
            release_archive_name("0.3.7", "x86_64-pc-windows-msvc"),
            "ori-v0.3.7-x86_64-pc-windows-msvc.zip"
        );
    }

    #[test]
    fn digest_field_yields_expected_sha256() {
        let asset = ReleaseAsset {
            name: "pkg.tar.gz".to_string(),
            browser_download_url: String::new(),
            digest: Some("sha256:abc123".to_string()),
        };
        assert_eq!(expected_sha256(&asset).unwrap(), "abc123");

        let missing = ReleaseAsset {
            digest: None,
            ..asset
        };
        let error = expected_sha256(&missing).unwrap_err();
        assert!(error.contains("update.checksum_missing"), "{error}");
    }

    #[test]
    fn packaged_install_is_found_in_flat_and_bin_layouts() {
        let flat = unique_temp_dir("flat_layout");
        std::fs::create_dir_all(flat.join("runtime")).unwrap();
        let install = locate_packaged_install(&flat.join("ori")).unwrap();
        assert_eq!(install.root, flat);

        let packaged = unique_temp_dir("bin_layout");
        std::fs::create_dir_all(packaged.join("runtime")).unwrap();
        std::fs::create_dir_all(packaged.join("bin")).unwrap();
        let install = locate_packaged_install(&packaged.join("bin").join("ori")).unwrap();
        assert_eq!(install.root, packaged);

        let dev = unique_temp_dir("dev_layout");
        let error = locate_packaged_install(&dev.join("target").join("ori")).unwrap_err();
        assert!(error.contains("update.unpackaged_install"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn system_package_roots_are_refused() {
        let error = refuse_system_package_install(Path::new("/usr/lib/ori")).unwrap_err();
        assert!(error.contains("update.system_package_install"), "{error}");
        assert!(refuse_system_package_install(Path::new("/usr/local/ori")).is_ok());
        assert!(refuse_system_package_install(Path::new("/home/dev/.local/ori")).is_ok());
    }
}
