use std::collections::{BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;

const LOCKFILE_FORMAT_VERSION: u32 = 2;
const CACHE_METADATA_FILE: &str = ".ori-cache.json";
const MAX_PACKAGE_DOWNLOAD_BYTES: u64 = 64 * 1024 * 1024;
const MAX_REGISTRY_METADATA_BYTES: u64 = 64 * 1024;
const MAX_PACKAGE_EXPANDED_BYTES: u64 = 256 * 1024 * 1024;
const MAX_PACKAGE_ARCHIVE_DEPTH: usize = 64;
const INSECURE_REGISTRY_ENV: &str = "ORI_ALLOW_INSECURE_REGISTRY";
const OFFLINE_ENV: &str = "ORI_OFFLINE";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageManifest {
    pub root: PathBuf,
    pub manifest_path: PathBuf,
    pub name: String,
    pub version: String,
    pub entry: PathBuf,
    pub ori_version: String,
    pub description: Option<String>,
    pub dependencies: Vec<PackageDependency>,
    pub native_libs: Vec<String>,
    pub native_config: NativeConfig,
    pub declared_features: BTreeSet<String>,
    pub default_features: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativeConfig {
    pub dependencies: Vec<NativeDependency>,
    pub platforms: NativePlatformConfigs,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeDependency {
    pub name: String,
    pub pkg_config: Option<String>,
    pub is_static: bool,
    pub framework: Option<String>,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct NativePlatformConfigs {
    pub linux: PlatformLinkConfig,
    pub windows: PlatformLinkConfig,
    pub macos: PlatformLinkConfig,
    pub all: PlatformLinkConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PlatformLinkConfig {
    pub libraries: Vec<String>,
    pub frameworks: Vec<String>,
    pub library_dirs: Vec<PathBuf>,
    pub link_flags: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageDependency {
    pub name: String,
    pub requirement: DependencyRequirement,
}

/// The reproducible dependency snapshot stored in `ori.lock`.
///
/// The manifest remains the human-edited source of requirements.  The lock
/// file records the concrete package versions (and Git revisions) that were
/// materialised, so a later build can detect drift before compiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageLockfile {
    pub root_name: String,
    pub root_version: String,
    pub dependencies: Vec<LockedDependency>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockedDependency {
    pub name: String,
    pub source: LockedDependencySource,
    pub version: String,
    pub path: Option<String>,
    pub url: Option<String>,
    pub revision: Option<String>,
    /// SHA-256 of the normalized package tree, excluding cache metadata.
    pub digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockedDependencySource {
    Registry,
    Path,
    Git,
}

#[derive(Debug, Clone)]
pub struct LockPackageOptions {
    pub path: PathBuf,
    pub locked: bool,
    pub cache_root: Option<PathBuf>,
    /// Refuse every network request while validating/restoring this lock.
    pub offline: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockPackageOutput {
    pub path: PathBuf,
    pub dependencies: usize,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DependencyRequirement {
    Version(String),
    Path {
        path: PathBuf,
        version: Option<String>,
    },
    /// Remote (or local `file://` / path) Git source. Fetched into the package cache.
    Git {
        url: String,
        /// Preferred pin (exactly one of rev / tag / branch should be set; default branch = `main`).
        rev: Option<String>,
        tag: Option<String>,
        branch: Option<String>,
        /// Optional expected package version from the cloned `ori.pkg.toml`.
        version: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDependencySpec {
    pub url: String,
    pub rev: Option<String>,
    pub tag: Option<String>,
    pub branch: Option<String>,
    pub version: Option<String>,
}

impl GitDependencySpec {
    pub fn from_requirement(
        url: String,
        rev: Option<String>,
        tag: Option<String>,
        branch: Option<String>,
        version: Option<String>,
    ) -> Self {
        Self {
            url,
            rev,
            tag,
            branch,
            version,
        }
    }

    pub fn ref_key(&self) -> String {
        if let Some(rev) = &self.rev {
            return format!("rev-{}", sanitize_cache_segment(rev));
        }
        if let Some(tag) = &self.tag {
            return format!("tag-{}", sanitize_cache_segment(tag));
        }
        if let Some(branch) = &self.branch {
            return format!("branch-{}", sanitize_cache_segment(branch));
        }
        "branch-main".to_string()
    }

    pub fn checkout_ref(&self) -> Option<String> {
        self.rev
            .clone()
            .or_else(|| self.tag.clone())
            .or_else(|| self.branch.clone())
    }
}

#[derive(Debug, Clone)]
pub struct InstallPackageOptions {
    pub name: String,
    pub source: Option<PathBuf>,
    pub cache_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledPackage {
    pub name: String,
    pub version: String,
    pub source_root: PathBuf,
    pub installed_root: PathBuf,
    pub already_installed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallPackageOutput {
    pub cache_root: PathBuf,
    pub packages: Vec<InstalledPackage>,
}

#[derive(Debug, Clone)]
pub struct GetDependenciesOptions {
    /// Project root, `ori.proj`, `ori.pkg.toml`, or package directory.
    pub path: PathBuf,
    pub cache_root: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GetDependenciesOutput {
    pub cache_root: PathBuf,
    pub packages: Vec<InstalledPackage>,
}

pub fn run_install_package(options: InstallPackageOptions) -> Result<InstallPackageOutput, String> {
    let cache_root = match options.cache_root {
        Some(path) => path,
        None => default_package_cache_root()?,
    };

    let mut seen = HashSet::new();
    let mut packages = Vec::new();

    // Registry install: `ori install name` or `ori install name@version` (no --path).
    if options.source.is_none()
        && !options.name.starts_with("github.com/")
        && !options.name.starts_with("https://")
        && !options.name.starts_with("http://")
        && !options.name.starts_with("git@")
    {
        let (pkg_name, version_opt) = split_name_version(&options.name);
        let registry = resolve_registry_location(None)?;
        let version = match version_opt {
            Some(v) => v.to_string(),
            None => latest_registry_version(&registry, pkg_name)?,
        };
        let installed_root =
            ensure_version_package_cached(pkg_name, &version, &cache_root, Some(&registry))?;
        install_local_package(
            &installed_root,
            Some(pkg_name),
            &cache_root,
            &mut seen,
            &mut packages,
        )?;
        return Ok(InstallPackageOutput {
            cache_root,
            packages,
        });
    }

    let source = match options.source {
        Some(path) => path,
        None => {
            if options.name.starts_with("github.com/")
                || options.name.starts_with("https://")
                || options.name.starts_with("http://")
            {
                let url = if options.name.starts_with("github.com/") {
                    format!("https://{}", options.name)
                } else {
                    options.name.clone()
                };
                let temp_dir = std::env::temp_dir().join(format!(
                    "ori_git_clone_{}",
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                ));

                eprintln!("ori: cloning {}...", url);
                let status = Command::new("git")
                    .arg("clone")
                    .arg("--depth")
                    .arg("1")
                    .arg(&url)
                    .arg(&temp_dir)
                    .status()
                    .map_err(|err| {
                        format!("package.git_clone_failed: failed to invoke git: {err}")
                    })?;

                if !status.success() {
                    return Err(format!(
                        "package.git_clone_failed: git clone failed with status: {}",
                        status
                    ));
                }

                temp_dir
            } else {
                return Err(format!(
                    "package.registry_unavailable: cannot fetch `{}`; set ORI_REGISTRY, use `ori install {} --path <dir>`, or a GitHub URL",
                    options.name, options.name
                ));
            }
        }
    };

    let expected_name =
        if options.name.starts_with("github.com/") || options.name.starts_with("http") {
            None
        } else {
            Some(options.name.as_str())
        };

    install_local_package(
        &package_root_from_path(&source)?,
        expected_name,
        &cache_root,
        &mut seen,
        &mut packages,
    )?;

    Ok(InstallPackageOutput {
        cache_root,
        packages,
    })
}

pub fn load_package_manifest(path: impl AsRef<Path>) -> Result<PackageManifest, String> {
    let input = path.as_ref();
    let manifest_path = if input.is_file() {
        input.to_path_buf()
    } else {
        input.join("ori.pkg.toml")
    };

    let source = fs::read_to_string(&manifest_path).map_err(|err| {
        format!(
            "package.manifest_missing: cannot read `{}`: {err}",
            manifest_path.display()
        )
    })?;
    let root = manifest_path
        .parent()
        .ok_or_else(|| {
            format!(
                "package.manifest_invalid: manifest `{}` has no parent directory",
                manifest_path.display()
            )
        })?
        .to_path_buf();

    parse_package_manifest(&source, root, manifest_path)
}

/// Return the lockfile path belonging to a package root.
pub fn package_lock_path(root: &Path) -> PathBuf {
    root.join("ori.lock")
}

/// Read an existing lockfile.  The parser intentionally accepts only the
/// small, stable format emitted by `write_package_lock`; malformed files fail
/// early instead of being silently ignored.
pub fn load_package_lock(path: &Path) -> Result<PackageLockfile, String> {
    let source = fs::read_to_string(path).map_err(|err| {
        format!(
            "package.lock_read_failed: cannot read `{}`: {err}",
            path.display()
        )
    })?;
    parse_package_lock(&source, path)
}

/// Resolve and write the lockfile for a package.  `locked` performs a
/// read-only validation and fails if the manifest would produce a different
/// snapshot.
pub fn run_lock_package(options: LockPackageOptions) -> Result<LockPackageOutput, String> {
    let root = package_root_from_path(&options.path)?;
    let manifest = load_lock_manifest(&root)?;
    let cache_root = options
        .cache_root
        .or_else(|| default_package_cache_root().ok());
    let lock_path = package_lock_path(&root);
    if options.locked {
        if !lock_path.is_file() {
            return Err(format!(
                "package.lock_missing: `{}` is required by `--locked`",
                lock_path.display()
            ));
        }
        let current = load_package_lock(&lock_path)?;
        validate_locked_manifest(&manifest, &current)?;
        restore_locked_dependencies(&root, &current, cache_root.as_deref(), options.offline)?;
        return Ok(LockPackageOutput {
            path: lock_path,
            dependencies: current.dependencies.len(),
            changed: false,
        });
    }

    let resolved = resolve_package_lock(&manifest, cache_root.as_deref())?;
    if lock_path.is_file() {
        match load_package_lock(&lock_path) {
            Ok(current) if current == resolved => {
                return Ok(LockPackageOutput {
                    path: lock_path,
                    dependencies: resolved.dependencies.len(),
                    changed: false,
                });
            }
            Ok(_) => {}
            // Format 1 has no content digests. An explicit unlocked `ori lock`
            // is the safe migration path; `--locked` still refuses it above.
            Err(error) if error.starts_with("package.lock_version:") => {}
            Err(error) => return Err(error),
        }
    }
    write_package_lock(&lock_path, &resolved)?;
    Ok(LockPackageOutput {
        path: lock_path,
        dependencies: resolved.dependencies.len(),
        changed: true,
    })
}

/// Validate a package lock when one is present.  Existing projects without a
/// lockfile retain their pre-lock behaviour; `ori lock` is the explicit opt-in
/// that creates the reproducible snapshot.
pub fn validate_package_lock(root: &Path) -> Result<(), String> {
    let lock_path = package_lock_path(root);
    if !lock_path.is_file() {
        return Ok(());
    }
    let manifest = load_lock_manifest(root)?;
    let cache = default_package_cache_root().ok();
    let current = load_package_lock(&lock_path)?;
    validate_locked_manifest(&manifest, &current)?;
    restore_locked_dependencies(root, &current, cache.as_deref(), env_flag(OFFLINE_ENV))
}

fn validate_locked_manifest(
    manifest: &PackageManifest,
    lock: &PackageLockfile,
) -> Result<(), String> {
    if manifest.name != lock.root_name || manifest.version != lock.root_version {
        return Err(format!(
            "package.lock_out_of_date: lock root `{}@{}` does not match manifest `{}@{}`",
            lock.root_name, lock.root_version, manifest.name, manifest.version
        ));
    }
    let mut unique_names = HashSet::new();
    for dependency in &lock.dependencies {
        if !unique_names.insert(&dependency.name) {
            return Err(format!(
                "package.lock_ambiguous: dependency `{}` appears more than once; source-specific aliases are required",
                dependency.name
            ));
        }
    }
    validate_manifest_dependencies(manifest, lock)
}

fn validate_manifest_dependencies(
    manifest: &PackageManifest,
    lock: &PackageLockfile,
) -> Result<(), String> {
    for requirement in &manifest.dependencies {
        let locked = lock
            .dependencies
            .iter()
            .find(|dependency| dependency.name == requirement.name)
            .ok_or_else(|| {
                format!(
                    "package.lock_out_of_date: manifest dependency `{}` is absent from the lock",
                    requirement.name
                )
            })?;
        match &requirement.requirement {
            DependencyRequirement::Version(version)
                if locked.source == LockedDependencySource::Registry
                    && locked.version == *version => {}
            DependencyRequirement::Path { path, version }
                if locked.source == LockedDependencySource::Path
                    && locked.path.as_deref()
                        == Some(path.to_string_lossy().replace('\\', "/").as_str())
                    && version
                        .as_ref()
                        .is_none_or(|version| *version == locked.version) => {}
            DependencyRequirement::Git { url, version, .. }
                if locked.source == LockedDependencySource::Git
                    && locked.url.as_deref().map(normalize_git_url)
                        == Some(normalize_git_url(url))
                    && version
                        .as_ref()
                        .is_none_or(|version| *version == locked.version)
                    && locked
                        .revision
                        .as_ref()
                        .is_some_and(|revision| !revision.is_empty()) => {}
            _ => {
                return Err(format!(
                    "package.lock_out_of_date: locked source/version for `{}` no longer matches its manifest requirement",
                    requirement.name
                ));
            }
        }
    }
    Ok(())
}

fn restore_locked_dependencies(
    project_root: &Path,
    lock: &PackageLockfile,
    cache_root: Option<&Path>,
    offline: bool,
) -> Result<(), String> {
    let mut restored_roots = Vec::new();
    // Materialize fetched sources before path entries that may point inside a
    // registry/Git package tree. The lock remains sorted for deterministic
    // serialization; restore order is derived from source kind.
    for dependency in lock
        .dependencies
        .iter()
        .filter(|dependency| dependency.source != LockedDependencySource::Path)
        .chain(
            lock.dependencies
                .iter()
                .filter(|dependency| dependency.source == LockedDependencySource::Path),
        )
    {
        let root = match dependency.source {
            LockedDependencySource::Path => {
                let source = dependency.url.as_deref().ok_or_else(|| {
                    format!(
                        "package.lock_source_invalid: path dependency `{}` has no normalized source identity",
                        dependency.name
                    )
                })?;
                let (locked_path, anchor) = if let Some(path) = source.strip_prefix("cache-path:") {
                    let cache = cache_root.ok_or_else(|| {
                        format!(
                            "package.cache_home_missing: set ORI_PACKAGE_CACHE to restore path dependency `{}` anchored in a fetched package",
                            dependency.name
                        )
                    })?;
                    (PathBuf::from(path), cache)
                } else {
                    (
                        source
                            .strip_prefix("path:")
                            .map(PathBuf::from)
                            .unwrap_or_else(|| {
                                PathBuf::from(dependency.path.as_deref().unwrap_or_default())
                            }),
                        project_root,
                    )
                };
                let path = if locked_path.is_absolute() {
                    // Accept early v2 development locks, but newly generated
                    // locks keep path sources project-relative and portable.
                    locked_path
                } else {
                    anchor.join(locked_path)
                };
                let manifest = load_package_manifest(&path)?;
                if manifest.name != dependency.name || manifest.version != dependency.version {
                    return Err(format!(
                        "package.lock_source_mismatch: path dependency `{}` now declares `{}@{}`",
                        dependency.name, manifest.name, manifest.version
                    ));
                }
                let actual = package_tree_digest(&path)?;
                if !actual.eq_ignore_ascii_case(&dependency.digest) {
                    return Err(format!(
                        "package.lock_digest_mismatch: path dependency `{}` hashed to {actual}, lock requires {}",
                        dependency.name, dependency.digest
                    ));
                }
                path
            }
            LockedDependencySource::Registry => {
                let cache = cache_root.ok_or_else(|| {
                    format!(
                        "package.cache_home_missing: set ORI_PACKAGE_CACHE to restore locked registry dependency `{}`",
                        dependency.name
                    )
                })?;
                let source = dependency.url.as_deref().ok_or_else(|| {
                    format!(
                        "package.lock_source_invalid: registry dependency `{}` has no source identity",
                        dependency.name
                    )
                })?;
                let target = cache.join(&dependency.name).join(&dependency.version);
                if target.join("ori.pkg.toml").is_file() {
                    validate_cache_entry(
                        &target,
                        &dependency.name,
                        &dependency.version,
                        Some(source),
                        Some(&dependency.digest),
                    )?;
                    target
                } else {
                    if offline {
                        return Err(format!(
                            "package.offline_cache_miss: locked registry dependency `{}@{}` is absent",
                            dependency.name, dependency.version
                        ));
                    }
                    let registry = RegistryLocation::from_source_identity(source)?;
                    fetch_package_from_registry(
                        &dependency.name,
                        &dependency.version,
                        &registry,
                        cache,
                        Some(&dependency.digest),
                    )?
                }
            }
            LockedDependencySource::Git => {
                let cache = cache_root.ok_or_else(|| {
                    format!(
                        "package.cache_home_missing: set ORI_PACKAGE_CACHE to restore locked Git dependency `{}`",
                        dependency.name
                    )
                })?;
                let url = dependency.url.clone().ok_or_else(|| {
                    format!(
                        "package.lock_source_invalid: Git dependency `{}` has no URL",
                        dependency.name
                    )
                })?;
                let revision = dependency.revision.as_deref().ok_or_else(|| {
                    format!(
                        "package.lock_source_invalid: Git dependency `{}` has no exact revision",
                        dependency.name
                    )
                })?;
                let spec = GitDependencySpec {
                    url,
                    rev: Some(revision.to_string()),
                    tag: None,
                    branch: None,
                    version: Some(dependency.version.clone()),
                };
                ensure_git_dependency_cached_with_lock(
                    &dependency.name,
                    &spec,
                    cache,
                    Some(revision),
                    Some(&dependency.digest),
                    offline,
                )?
            }
        };
        restored_roots.push(root);
    }
    for root in restored_roots {
        validate_manifest_dependencies(&load_package_manifest(root)?, lock)?;
    }
    Ok(())
}

fn load_lock_manifest(root: &Path) -> Result<PackageManifest, String> {
    let package_manifest = root.join("ori.pkg.toml");
    if package_manifest.is_file() {
        return load_package_manifest(&package_manifest);
    }
    let project_manifest = root.join("ori.proj");
    if project_manifest.is_file() {
        return parse_project_lock_manifest(&project_manifest);
    }
    Err(format!(
        "package.get_target_invalid: `{}` has neither `ori.proj` nor `ori.pkg.toml`",
        root.display()
    ))
}

fn parse_project_lock_manifest(path: &Path) -> Result<PackageManifest, String> {
    let source = fs::read_to_string(path).map_err(|err| {
        format!(
            "package.manifest_missing: cannot read `{}`: {err}",
            path.display()
        )
    })?;
    let root = path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let mut section = String::new();
    let mut name = None;
    let mut version = None;
    let mut entry = None;
    let mut dependencies = Vec::new();
    for (line_index, raw_line) in source.lines().enumerate() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }
        let Some((key, value)) = split_key_value(line) else {
            return Err(format!(
                "package.manifest_syntax: `{}` line {}: expected `key = value`",
                path.display(),
                line_index + 1
            ));
        };
        if section == "dependencies" {
            dependencies.push(parse_dependency(key, value).map_err(|err| {
                format!(
                    "package.manifest_syntax: `{}` line {}: {err}",
                    path.display(),
                    line_index + 1
                )
            })?);
            continue;
        }
        match key.trim() {
            "name" => name = Some(parse_string_value(value)?),
            "version" => version = Some(parse_string_value(value)?),
            "entry" => entry = Some(parse_string_value(value)?),
            _ => {}
        }
    }
    let name = name.unwrap_or_else(|| {
        root.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("ori-project")
            .to_string()
    });
    let version = version.unwrap_or_else(|| "0.0.0".to_string());
    validate_package_name(&name).map_err(|err| format!("package.name_invalid: {err}"))?;
    validate_semver_like(&version).map_err(|err| format!("package.version_invalid: {err}"))?;
    Ok(PackageManifest {
        root: root.clone(),
        manifest_path: path.to_path_buf(),
        name,
        version,
        entry: root.join(entry.unwrap_or_else(|| "main.orl".to_string())),
        ori_version: "0.3.8".to_string(),
        description: None,
        dependencies,
        native_libs: Vec::new(),
        native_config: NativeConfig::default(),
        declared_features: BTreeSet::new(),
        default_features: BTreeSet::new(),
    })
}

fn resolve_package_lock(
    manifest: &PackageManifest,
    cache_root: Option<&Path>,
) -> Result<PackageLockfile, String> {
    let mut dependencies = Vec::new();
    let mut seen = HashSet::new();
    collect_locked_dependencies(
        manifest,
        &manifest.root,
        cache_root,
        &mut seen,
        &mut dependencies,
    )?;
    dependencies.sort_by(|a, b| a.name.cmp(&b.name).then(a.version.cmp(&b.version)));
    Ok(PackageLockfile {
        root_name: manifest.name.clone(),
        root_version: manifest.version.clone(),
        dependencies,
    })
}

fn collect_locked_dependencies(
    manifest: &PackageManifest,
    project_root: &Path,
    cache_root: Option<&Path>,
    seen: &mut HashSet<String>,
    locked: &mut Vec<LockedDependency>,
) -> Result<(), String> {
    for dependency in &manifest.dependencies {
        let (root, entry) = match &dependency.requirement {
            DependencyRequirement::Path { path, version } => {
                let root = manifest.root.join(path);
                let child = load_package_manifest(&root)?;
                if child.name != dependency.name {
                    return Err(format!(
                        "package.dependency_name_mismatch: dependency `{}` points to package `{}`",
                        dependency.name, child.name
                    ));
                }
                if let Some(expected) = version {
                    if child.version != *expected {
                        return Err(format!(
                            "package.dependency_version_mismatch: dependency `{}` expected `{expected}`, found `{}`",
                            dependency.name, child.version
                        ));
                    }
                }
                let relative = path.to_string_lossy().replace('\\', "/");
                let digest = package_tree_digest(&root)?;
                let source_identity =
                    relative_path_source_identity(project_root, cache_root, &root)?;
                (
                    root,
                    LockedDependency {
                        name: child.name.clone(),
                        source: LockedDependencySource::Path,
                        version: child.version.clone(),
                        path: Some(relative),
                        url: Some(source_identity),
                        revision: None,
                        digest,
                    },
                )
            }
            DependencyRequirement::Version(version) => {
                let cache = cache_root.ok_or_else(|| {
                    format!(
                        "package.cache_home_missing: set ORI_PACKAGE_CACHE to resolve version dependency `{}`",
                        dependency.name
                    )
                })?;
                let root = ensure_version_package_cached(
                    &dependency.name,
                    version,
                    cache,
                    resolve_registry_location(None).ok().as_ref(),
                )?;
                let child = load_package_manifest(&root)?;
                let metadata = read_cache_metadata(&root)?;
                (
                    root,
                    LockedDependency {
                        name: child.name.clone(),
                        source: LockedDependencySource::Registry,
                        version: child.version.clone(),
                        path: None,
                        url: Some(metadata.source),
                        revision: None,
                        digest: metadata.digest,
                    },
                )
            }
            DependencyRequirement::Git {
                url,
                rev,
                tag,
                branch,
                version,
            } => {
                let cache = cache_root.ok_or_else(|| {
                    "package.cache_home_missing: set ORI_PACKAGE_CACHE to resolve git dependencies"
                        .to_string()
                })?;
                let spec = GitDependencySpec::from_requirement(
                    url.clone(),
                    rev.clone(),
                    tag.clone(),
                    branch.clone(),
                    version.clone(),
                );
                let root = ensure_git_dependency_cached(&dependency.name, &spec, cache)?;
                let child = load_package_manifest(&root)?;
                let metadata = read_cache_metadata(&root)?;
                let revision = metadata
                    .source
                    .rsplit_once('#')
                    .map(|(_, revision)| revision.to_string())
                    .or_else(|| rev.clone());
                (
                    root,
                    LockedDependency {
                        name: child.name.clone(),
                        source: LockedDependencySource::Git,
                        version: child.version.clone(),
                        path: None,
                        url: Some(url.clone()),
                        revision,
                        digest: metadata.digest,
                    },
                )
            }
        };

        let key = format!("{}@{}", entry.name, entry.version);
        if seen.insert(key) {
            collect_locked_dependencies(
                &load_package_manifest(&root)?,
                project_root,
                cache_root,
                seen,
                locked,
            )?;
            locked.push(entry);
        }
    }
    Ok(())
}

fn git_revision(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .arg("rev-parse")
        .arg("HEAD")
        .output()
        .map_err(|err| format!("git rev-parse failed: {err}"))?;
    if !output.status.success() {
        return Err("git rev-parse returned a failure status".to_string());
    }
    let revision = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if revision.is_empty() {
        Err("git rev-parse returned an empty revision".to_string())
    } else {
        Ok(revision)
    }
}

fn write_package_lock(path: &Path, lock: &PackageLockfile) -> Result<(), String> {
    let mut text = String::from("# This file is generated by `ori lock`.\n");
    text.push_str(&format!("format = {LOCKFILE_FORMAT_VERSION}\n"));
    text.push_str(&format!("root = {}\n", quote_lock_string(&lock.root_name)));
    text.push_str(&format!(
        "root_version = {}\n\n",
        quote_lock_string(&lock.root_version)
    ));
    for dependency in &lock.dependencies {
        text.push_str("[[dependency]]\n");
        text.push_str(&format!("name = {}\n", quote_lock_string(&dependency.name)));
        text.push_str(&format!(
            "source = {}\n",
            quote_lock_string(match dependency.source {
                LockedDependencySource::Registry => "registry",
                LockedDependencySource::Path => "path",
                LockedDependencySource::Git => "git",
            })
        ));
        text.push_str(&format!(
            "version = {}\n",
            quote_lock_string(&dependency.version)
        ));
        if let Some(path_value) = &dependency.path {
            text.push_str(&format!("path = {}\n", quote_lock_string(path_value)));
        }
        if let Some(url) = &dependency.url {
            text.push_str(&format!("url = {}\n", quote_lock_string(url)));
        }
        if let Some(revision) = &dependency.revision {
            text.push_str(&format!("revision = {}\n", quote_lock_string(revision)));
        }
        text.push_str(&format!(
            "digest = {}\n",
            quote_lock_string(&dependency.digest)
        ));
        text.push('\n');
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temporary_dir = ExclusiveTempDir::create(parent, "lock-write")?;
    let temporary = temporary_dir.path.join("ori.lock");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|err| {
            format!(
                "package.lock_write_failed: cannot exclusively create `{}`: {err}",
                temporary.display()
            )
        })?;
    file.write_all(text.as_bytes()).map_err(|err| {
        format!(
            "package.lock_write_failed: cannot write `{}`: {err}",
            path.display()
        )
    })?;
    file.sync_all().map_err(|err| {
        format!(
            "package.lock_write_failed: cannot sync `{}`: {err}",
            path.display()
        )
    })?;
    drop(file);
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(rename_error) if cfg!(windows) && path.is_file() => {
            // Windows does not replace an existing destination with rename.
            // The lockfile is generated state, so replace it explicitly only
            // after the atomic rename attempt has failed.
            fs::remove_file(path)
                .and_then(|_| fs::rename(&temporary, path))
                .map_err(|replace_error| {
                    format!(
                        "package.lock_write_failed: cannot replace `{}`: {rename_error}; retry failed: {replace_error}",
                        path.display()
                    )
                })
        }
        Err(err) => Err(format!(
            "package.lock_write_failed: cannot replace `{}`: {err}",
            path.display()
        )),
    }
}

fn parse_package_lock(source: &str, path: &Path) -> Result<PackageLockfile, String> {
    let mut format = None;
    let mut root = None;
    let mut root_version = None;
    let mut dependencies = Vec::new();
    let mut current: Option<HashMap<String, String>> = None;
    for (line_index, raw_line) in source.lines().enumerate() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[dependency]]" {
            if let Some(table) = current.take() {
                dependencies.push(parse_locked_dependency(table, path, line_index + 1)?);
            }
            current = Some(HashMap::new());
            continue;
        }
        let (key, value) = split_key_value(line).ok_or_else(|| {
            format!(
                "package.lock_syntax: `{}` line {}: expected `key = value`",
                path.display(),
                line_index + 1
            )
        })?;
        let key = key.trim();
        if let Some(table) = current.as_mut() {
            table.insert(
                key.to_string(),
                parse_string_value(value).map_err(|err| {
                    format!(
                        "package.lock_syntax: `{}` line {}: {err}",
                        path.display(),
                        line_index + 1
                    )
                })?,
            );
        } else {
            match key {
                "format" => {
                    format = Some(value.trim().parse::<u32>().map_err(|_| {
                        format!(
                            "package.lock_syntax: `{}` has invalid format",
                            path.display()
                        )
                    })?);
                }
                "root" => root = Some(parse_string_value(value)?),
                "root_version" => root_version = Some(parse_string_value(value)?),
                other => {
                    return Err(format!(
                        "package.lock_syntax: `{}` has unknown top-level key `{other}`",
                        path.display()
                    ));
                }
            }
        }
    }
    if let Some(table) = current.take() {
        dependencies.push(parse_locked_dependency(
            table,
            path,
            source.lines().count(),
        )?);
    }
    if format != Some(LOCKFILE_FORMAT_VERSION) {
        return Err(format!(
            "package.lock_version: `{}` uses an unsupported lockfile format",
            path.display()
        ));
    }
    Ok(PackageLockfile {
        root_name: root.ok_or_else(|| {
            format!(
                "package.lock_syntax: `{}` is missing `root`",
                path.display()
            )
        })?,
        root_version: root_version.ok_or_else(|| {
            format!(
                "package.lock_syntax: `{}` is missing `root_version`",
                path.display()
            )
        })?,
        dependencies,
    })
}

fn parse_locked_dependency(
    table: HashMap<String, String>,
    path: &Path,
    line: usize,
) -> Result<LockedDependency, String> {
    let required = |key: &str| {
        table.get(key).cloned().ok_or_else(|| {
            format!(
                "package.lock_syntax: `{}` dependency near line {} is missing `{key}`",
                path.display(),
                line
            )
        })
    };
    let source = match required("source")?.as_str() {
        "registry" => LockedDependencySource::Registry,
        "path" => LockedDependencySource::Path,
        "git" => LockedDependencySource::Git,
        other => {
            return Err(format!(
                "package.lock_syntax: `{}` has unknown dependency source `{other}`",
                path.display()
            ));
        }
    };
    let digest = required("digest")?;
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "package.lock_syntax: `{}` dependency near line {} has invalid SHA-256 digest",
            path.display(),
            line
        ));
    }
    Ok(LockedDependency {
        name: required("name")?,
        source,
        version: required("version")?,
        path: table.get("path").cloned(),
        url: table.get("url").cloned(),
        revision: table.get("revision").cloned(),
        digest: digest.to_ascii_lowercase(),
    })
}

fn quote_lock_string(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn install_local_package(
    root: &Path,
    expected_name: Option<&str>,
    cache_root: &Path,
    seen: &mut HashSet<String>,
    packages: &mut Vec<InstalledPackage>,
) -> Result<(), String> {
    let manifest = load_package_manifest(root)?;
    if let Some(expected) = expected_name {
        if manifest.name != expected {
            return Err(format!(
                "package.name_mismatch: requested `{expected}`, but `{}` declares `{}`",
                manifest.manifest_path.display(),
                manifest.name
            ));
        }
    }

    let key = format!("{}@{}", manifest.name, manifest.version);
    if !seen.insert(key) {
        return Ok(());
    }

    for dependency in &manifest.dependencies {
        match &dependency.requirement {
            DependencyRequirement::Version(version) => {
                let root = ensure_version_package_cached(
                    &dependency.name,
                    version,
                    cache_root,
                    resolve_registry_location(None).ok().as_ref(),
                )?;
                install_local_package(&root, Some(&dependency.name), cache_root, seen, packages)?;
            }
            DependencyRequirement::Path { path, version } => {
                let dependency_root = manifest.root.join(path);
                let dependency_manifest = load_package_manifest(&dependency_root)?;
                if dependency_manifest.name != dependency.name {
                    return Err(format!(
                        "package.dependency_name_mismatch: dependency `{}` points to package `{}`",
                        dependency.name, dependency_manifest.name
                    ));
                }
                if let Some(expected_version) = version {
                    if dependency_manifest.version != *expected_version {
                        return Err(format!(
                            "package.dependency_version_mismatch: dependency `{}` expected `{expected_version}`, found `{}`",
                            dependency.name, dependency_manifest.version
                        ));
                    }
                }
                install_local_package(
                    &dependency_root,
                    Some(&dependency.name),
                    cache_root,
                    seen,
                    packages,
                )?;
            }
            DependencyRequirement::Git {
                url,
                rev,
                tag,
                branch,
                version,
            } => {
                let spec = GitDependencySpec::from_requirement(
                    url.clone(),
                    rev.clone(),
                    tag.clone(),
                    branch.clone(),
                    version.clone(),
                );
                let dependency_root =
                    ensure_git_dependency_cached(&dependency.name, &spec, cache_root)?;
                install_local_package(
                    &dependency_root,
                    Some(&dependency.name),
                    cache_root,
                    seen,
                    packages,
                )?;
            }
        }
    }

    let target_root = cache_root.join(&manifest.name).join(&manifest.version);
    let already_installed = target_root.join("ori.pkg.toml").is_file();
    let source_identity =
        if fs::canonicalize(&manifest.root).ok() == fs::canonicalize(&target_root).ok() {
            None
        } else {
            Some(path_source_identity(&manifest.root)?)
        };
    if already_installed {
        validate_cache_entry(
            &target_root,
            &manifest.name,
            &manifest.version,
            source_identity.as_deref(),
            None,
        )?;
    } else {
        let source_identity = source_identity.ok_or_else(|| {
            format!(
                "package.cache_conflict: cache target `{}` is incomplete",
                target_root.display()
            )
        })?;
        publish_cache_entry(&manifest.root, &target_root, &source_identity, None)?;
    }

    packages.push(InstalledPackage {
        name: manifest.name,
        version: manifest.version,
        source_root: manifest.root,
        installed_root: target_root,
        already_installed,
    });

    Ok(())
}

fn path_source_identity(path: &Path) -> Result<String, String> {
    let canonical = fs::canonicalize(path).map_err(|err| {
        format!(
            "package.source_read_failed: cannot canonicalize `{}`: {err}",
            path.display()
        )
    })?;
    Ok(format!(
        "path:{}",
        canonical.to_string_lossy().replace('\\', "/")
    ))
}

fn relative_path_source_identity(
    project_root: &Path,
    cache_root: Option<&Path>,
    source: &Path,
) -> Result<String, String> {
    let project_root = fs::canonicalize(project_root).map_err(|err| {
        format!(
            "package.source_read_failed: cannot canonicalize project root `{}`: {err}",
            project_root.display()
        )
    })?;
    let source = fs::canonicalize(source).map_err(|err| {
        format!(
            "package.source_read_failed: cannot canonicalize dependency `{}`: {err}",
            source.display()
        )
    })?;
    if let Some(cache_root) = cache_root.and_then(|path| fs::canonicalize(path).ok()) {
        if let Ok(cache_relative) = source.strip_prefix(&cache_root) {
            let cache_relative = cache_relative.to_str().ok_or_else(|| {
                format!(
                    "package.path_incompatible: cache path `{}` is not valid UTF-8",
                    source.display()
                )
            })?;
            return Ok(format!("cache-path:{}", cache_relative.replace('\\', "/")));
        }
    }
    let base_components = project_root.components().collect::<Vec<_>>();
    let source_components = source.components().collect::<Vec<_>>();
    let common = base_components
        .iter()
        .zip(&source_components)
        .take_while(|(left, right)| left == right)
        .count();
    if common == 0 {
        return Err(format!(
            "package.path_incompatible: `{}` and `{}` have no common filesystem root",
            project_root.display(),
            source.display()
        ));
    }
    let mut relative = PathBuf::new();
    for component in &base_components[common..] {
        if matches!(component, std::path::Component::Normal(_)) {
            relative.push("..");
        }
    }
    for component in &source_components[common..] {
        relative.push(component.as_os_str());
    }
    if relative.as_os_str().is_empty() {
        relative.push(".");
    }
    let relative = relative.to_str().ok_or_else(|| {
        format!(
            "package.path_incompatible: dependency path `{}` is not valid UTF-8",
            source.display()
        )
    })?;
    Ok(format!("path:{}", relative.replace('\\', "/")))
}

fn package_root_from_path(path: &Path) -> Result<PathBuf, String> {
    if path.is_file() {
        return path.parent().map(Path::to_path_buf).ok_or_else(|| {
            format!(
                "package.manifest_invalid: `{}` has no parent directory",
                path.display()
            )
        });
    }
    Ok(path.to_path_buf())
}

fn parse_package_manifest(
    source: &str,
    root: PathBuf,
    manifest_path: PathBuf,
) -> Result<PackageManifest, String> {
    let mut section = String::new();
    let mut package = HashMap::new();
    let mut dependencies = Vec::new();
    let mut native_libs = Vec::new();
    let mut native_config = NativeConfig::default();
    let mut sub_native_deps: HashMap<String, NativeDependency> = HashMap::new();
    let mut declared_features = BTreeSet::new();
    let mut default_features = BTreeSet::new();

    for (line_index, raw_line) in source.lines().enumerate() {
        let line_no = line_index + 1;
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') {
            if !line.ends_with(']') {
                return Err(format!(
                    "package.manifest_syntax: `{}` line {line_no}: unterminated section",
                    manifest_path.display()
                ));
            }
            section = line[1..line.len() - 1].trim().to_string();
            continue;
        }

        let (key, value) = split_key_value(line).ok_or_else(|| {
            format!(
                "package.manifest_syntax: `{}` line {line_no}: expected `key = value`",
                manifest_path.display()
            )
        })?;

        if section == "native" {
            parse_platform_link_key(
                &mut native_config.platforms.all,
                key,
                value,
                &manifest_path,
                line_no,
            )?;
            continue;
        }
        if let Some(target) = section.strip_prefix("native.") {
            match target {
                "linux" => {
                    parse_platform_link_key(
                        &mut native_config.platforms.linux,
                        key,
                        value,
                        &manifest_path,
                        line_no,
                    )?;
                    continue;
                }
                "windows" => {
                    parse_platform_link_key(
                        &mut native_config.platforms.windows,
                        key,
                        value,
                        &manifest_path,
                        line_no,
                    )?;
                    continue;
                }
                "macos" => {
                    parse_platform_link_key(
                        &mut native_config.platforms.macos,
                        key,
                        value,
                        &manifest_path,
                        line_no,
                    )?;
                    continue;
                }
                "dependencies" => {
                    let dep = parse_native_dependency_entry(key, value, &manifest_path, line_no)?;
                    native_config.dependencies.push(dep);
                    continue;
                }
                _ if target.starts_with("dependencies.") => {
                    let dep_name = target["dependencies.".len()..].trim();
                    if dep_name.is_empty() {
                        return Err(format!(
                            "package.manifest_syntax: `{}` line {line_no}: empty native dependency name in section",
                            manifest_path.display()
                        ));
                    }
                    let entry = sub_native_deps
                        .entry(dep_name.to_string())
                        .or_insert_with(|| NativeDependency {
                            name: dep_name.to_string(),
                            pkg_config: None,
                            is_static: false,
                            framework: None,
                            version: None,
                        });
                    parse_native_dep_field(entry, key, value, &manifest_path, line_no)?;
                    continue;
                }
                _ => {}
            }
        }

        match section.as_str() {
            "package" => {
                let key = normalize_key(key)?;
                if key == "authors" {
                    continue;
                }
                if key == "native_libs" {
                    native_libs = parse_string_array_value(value).map_err(|err| {
                        format!(
                            "package.manifest_syntax: `{}` line {line_no}: {err}",
                            manifest_path.display()
                        )
                    })?;
                    continue;
                }
                let value = parse_string_value(value).map_err(|err| {
                    format!(
                        "package.manifest_syntax: `{}` line {line_no}: {err}",
                        manifest_path.display()
                    )
                })?;
                package.insert(key, value);
            }
            "dependencies" => {
                dependencies.push(parse_dependency(key, value).map_err(|err| {
                    format!(
                        "package.manifest_syntax: `{}` line {line_no}: {err}",
                        manifest_path.display()
                    )
                })?);
            }
            "features" => {
                let feature = normalize_key(key)?;
                let values = parse_string_array_value(value).map_err(|err| {
                    format!(
                        "package.manifest_syntax: `{}` line {line_no}: {err}",
                        manifest_path.display()
                    )
                })?;
                if feature == "default" {
                    default_features.extend(values);
                } else {
                    validate_feature_identifier(&feature).map_err(|err| {
                        format!(
                            "package.manifest_syntax: `{}` line {line_no}: {err}",
                            manifest_path.display()
                        )
                    })?;
                    if !values.is_empty() {
                        return Err(format!(
                            "package.manifest_syntax: `{}` line {line_no}: feature `{feature}` must use an empty array in cfg v1",
                            manifest_path.display()
                        ));
                    }
                    if !declared_features.insert(feature.clone()) {
                        return Err(format!(
                            "package.manifest_syntax: `{}` line {line_no}: feature `{feature}` is declared more than once",
                            manifest_path.display()
                        ));
                    }
                }
            }
            "" => {
                return Err(format!(
                    "package.manifest_syntax: `{}` line {line_no}: values must be inside a section",
                    manifest_path.display()
                ));
            }
            other => {
                return Err(format!(
                    "package.manifest_syntax: `{}` line {line_no}: unsupported section `[{}]`",
                    manifest_path.display(),
                    other
                ));
            }
        }
    }

    let name = required_field(&package, "name", &manifest_path)?;
    validate_package_name(&name).map_err(|err| {
        format!(
            "package.name_invalid: `{}` declares invalid package name `{name}`: {err}",
            manifest_path.display()
        )
    })?;

    let version = required_field(&package, "version", &manifest_path)?;
    validate_semver_like(&version).map_err(|err| {
        format!(
            "package.version_invalid: `{}` declares invalid version `{version}`: {err}",
            manifest_path.display()
        )
    })?;

    let entry_raw = required_field(&package, "entry", &manifest_path)?;
    let entry = root.join(&entry_raw);
    if entry.extension().and_then(|ext| ext.to_str()) != Some("orl") {
        return Err(format!(
            "package.entry_invalid: `{}` entry must be a `.orl` file",
            manifest_path.display()
        ));
    }
    if !entry.is_file() {
        return Err(format!(
            "package.entry_missing: `{}` entry `{}` does not exist",
            manifest_path.display(),
            entry.display()
        ));
    }

    let ori_version = required_field(&package, "ori_version", &manifest_path)?;
    let description = package.get("description").cloned();
    if let Some(unknown) = default_features
        .iter()
        .find(|feature| !declared_features.contains(*feature))
    {
        return Err(format!(
            "package.manifest_syntax: `{}` enables undeclared default feature `{unknown}`",
            manifest_path.display()
        ));
    }

    for (_dep_name, dep) in sub_native_deps {
        if !native_config
            .dependencies
            .iter()
            .any(|d| d.name == dep.name)
        {
            native_config.dependencies.push(dep);
        }
    }

    Ok(PackageManifest {
        root,
        manifest_path,
        name,
        version,
        entry,
        ori_version,
        description,
        dependencies,
        native_libs,
        native_config,
        declared_features,
        default_features,
    })
}

fn validate_feature_identifier(name: &str) -> Result<(), String> {
    let mut chars = name.chars();
    let valid = chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric());
    if valid && name != "default" {
        Ok(())
    } else {
        Err(format!(
            "invalid feature name `{name}`; use an ASCII identifier other than `default`"
        ))
    }
}

fn parse_bool_value(raw: &str) -> Result<bool, String> {
    let value = raw.trim().trim_matches('"').trim();
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("expected `true` or `false`, found `{raw}`")),
    }
}

/// Parse `[native.*]`, `[native.linux]`, `[native.windows]`, `[native.macos]`
/// keys into a `PlatformLinkConfig`. Keys accepted:
/// `libraries`, `frameworks`, `library_dirs`, `link_flags`.
fn parse_platform_link_key(
    platform: &mut PlatformLinkConfig,
    key: &str,
    value: &str,
    manifest_path: &Path,
    line_no: usize,
) -> Result<(), String> {
    let key = normalize_key(key).map_err(|err| {
        format!(
            "package.manifest_syntax: `{}` line {line_no}: {err}",
            manifest_path.display()
        )
    })?;
    match key.as_str() {
        "libraries" => {
            let libs = parse_string_array_value(value).map_err(|err| {
                format!(
                    "package.manifest_syntax: `{}` line {line_no}: {err}",
                    manifest_path.display()
                )
            })?;
            platform.libraries.extend(libs);
        }
        "frameworks" => {
            let fws = parse_string_array_value(value).map_err(|err| {
                format!(
                    "package.manifest_syntax: `{}` line {line_no}: {err}",
                    manifest_path.display()
                )
            })?;
            platform.frameworks.extend(fws);
        }
        "library_dirs" => {
            let dirs = parse_string_array_value(value).map_err(|err| {
                format!(
                    "package.manifest_syntax: `{}` line {line_no}: {err}",
                    manifest_path.display()
                )
            })?;
            platform
                .library_dirs
                .extend(dirs.into_iter().map(PathBuf::from));
        }
        "link_flags" => {
            let flags = parse_string_array_value(value).map_err(|err| {
                format!(
                    "package.manifest_syntax: `{}` line {line_no}: {err}",
                    manifest_path.display()
                )
            })?;
            platform.link_flags.extend(flags);
        }
        other => {
            return Err(format!(
                "package.manifest_syntax: `{}` line {line_no}: unknown platform link key `{other}`",
                manifest_path.display()
            ));
        }
    }
    Ok(())
}

/// Parse a native dependency inline entry under `[native.dependencies]`.
/// Forms accepted: `name = "pkg-config"` or
/// `name = { pkg_config = "...", static = true/false, framework = "...", version = "..." }`.
fn parse_native_dependency_entry(
    key: &str,
    value: &str,
    manifest_path: &Path,
    line_no: usize,
) -> Result<NativeDependency, String> {
    let name = normalize_key(key).map_err(|err| {
        format!(
            "package.manifest_syntax: `{}` line {line_no}: {err}",
            manifest_path.display()
        )
    })?;
    let value = value.trim();
    if value.starts_with('{') {
        let table = parse_loose_inline_table(value).map_err(|err| {
            format!(
                "package.manifest_syntax: `{}` line {line_no}: {err}",
                manifest_path.display()
            )
        })?;
        let pkg_config = table.get("pkg_config").cloned();
        let is_static = table
            .get("static")
            .map(|s| parse_bool_value(s))
            .transpose()
            .map_err(|err| {
                format!(
                    "package.manifest_syntax: `{}` line {line_no}: {err}",
                    manifest_path.display()
                )
            })?
            .unwrap_or(false);
        let framework = table.get("framework").cloned();
        let version = table.get("version").cloned();
        Ok(NativeDependency {
            name,
            pkg_config,
            is_static,
            framework,
            version,
        })
    } else if value.starts_with('"') {
        let pkg_config = parse_string_value(value).map_err(|err| {
            format!(
                "package.manifest_syntax: `{}` line {line_no}: {err}",
                manifest_path.display()
            )
        })?;
        Ok(NativeDependency {
            name,
            pkg_config: Some(pkg_config),
            is_static: false,
            framework: None,
            version: None,
        })
    } else {
        Err(format!(
            "package.manifest_syntax: `{}` line {line_no}: expected inline table or string for native dependency `{name}`",
            manifest_path.display()
        ))
    }
}

/// Parse a field within a `[native.dependencies.<name>]` section.
fn parse_native_dep_field(
    dep: &mut NativeDependency,
    key: &str,
    value: &str,
    manifest_path: &Path,
    line_no: usize,
) -> Result<(), String> {
    let key = normalize_key(key).map_err(|err| {
        format!(
            "package.manifest_syntax: `{}` line {line_no}: {err}",
            manifest_path.display()
        )
    })?;
    match key.as_str() {
        "pkg_config" => {
            let pkg_config = parse_string_value(value).map_err(|err| {
                format!(
                    "package.manifest_syntax: `{}` line {line_no}: {err}",
                    manifest_path.display()
                )
            })?;
            dep.pkg_config = Some(pkg_config);
        }
        "static" => {
            dep.is_static = parse_bool_value(value).map_err(|err| {
                format!(
                    "package.manifest_syntax: `{}` line {line_no}: {err}",
                    manifest_path.display()
                )
            })?;
        }
        "framework" => {
            let fw = parse_string_value(value).map_err(|err| {
                format!(
                    "package.manifest_syntax: `{}` line {line_no}: {err}",
                    manifest_path.display()
                )
            })?;
            dep.framework = Some(fw);
        }
        "version" => {
            let version = parse_string_value(value).map_err(|err| {
                format!(
                    "package.manifest_syntax: `{}` line {line_no}: {err}",
                    manifest_path.display()
                )
            })?;
            dep.version = Some(version);
        }
        other => {
            return Err(format!(
                "package.manifest_syntax: `{}` line {line_no}: unknown native dependency key `{other}`",
                manifest_path.display()
            ));
        }
    }
    Ok(())
}

fn parse_loose_inline_table(raw: &str) -> Result<HashMap<String, String>, String> {
    let trimmed = raw.trim();
    if !trimmed.ends_with('}') {
        return Err("inline table must end with `}`".to_string());
    }
    let inner = trimmed
        .strip_prefix('{')
        .ok_or_else(|| "inline table must start with `{`".to_string())?
        .strip_suffix('}')
        .unwrap()
        .trim();
    let mut out = HashMap::new();
    if inner.is_empty() {
        return Ok(out);
    }
    for part in split_top_level(inner, ',') {
        let (key, value) = split_key_value(part.trim())
            .ok_or_else(|| "inline table item must use `key = value`".to_string())?;
        let key = normalize_key(key)?;
        let value = value.trim();
        let parsed_value = if value.starts_with('"') {
            parse_string_value(value)?
        } else {
            // Permite `true` / `false` sem aspas em tabelas inline
            value.to_string()
        };
        out.insert(key, parsed_value);
    }
    Ok(out)
}

fn parse_dependency(key: &str, value: &str) -> Result<PackageDependency, String> {
    let name = normalize_key(key)?;
    validate_package_name(&name)
        .map_err(|err| format!("dependency name `{name}` is invalid: {err}"))?;
    let value = value.trim();
    if value.starts_with('"') {
        let version = parse_string_value(value)?;
        validate_semver_like(&version)
            .map_err(|err| format!("dependency `{name}` version `{version}` is invalid: {err}"))?;
        return Ok(PackageDependency {
            name,
            requirement: DependencyRequirement::Version(version),
        });
    }
    if value.starts_with('{') {
        let table = parse_inline_table(value)?;
        let version = table.get("version").cloned();
        if let Some(version) = &version {
            validate_semver_like(version).map_err(|err| {
                format!("dependency `{name}` version `{version}` is invalid: {err}")
            })?;
        }

        if let Some(git) = table.get("git").cloned() {
            let rev = table.get("rev").cloned();
            let tag = table.get("tag").cloned();
            let branch = table.get("branch").cloned();
            let pin_count = [rev.is_some(), tag.is_some(), branch.is_some()]
                .into_iter()
                .filter(|v| *v)
                .count();
            if pin_count > 1 {
                return Err(format!(
                    "dependency `{name}` git table may set only one of `rev`, `tag`, or `branch`"
                ));
            }
            if table.contains_key("path") {
                return Err(format!(
                    "dependency `{name}` cannot combine `git` and `path` in the same table"
                ));
            }
            return Ok(PackageDependency {
                name,
                requirement: DependencyRequirement::Git {
                    url: git,
                    rev,
                    tag,
                    branch,
                    version,
                },
            });
        }

        let path = table
            .get("path")
            .cloned()
            .ok_or_else(|| {
                format!(
                    "dependency `{name}` table requires `path` or `git` (optional `version`/`rev`/`tag`/`branch`)"
                )
            })?;
        return Ok(PackageDependency {
            name,
            requirement: DependencyRequirement::Path {
                path: PathBuf::from(path),
                version,
            },
        });
    }
    Err(format!(
        "dependency `{name}` must be a version string, `{{ path = \"...\" }}`, or `{{ git = \"...\" }}` table"
    ))
}

fn parse_inline_table(value: &str) -> Result<HashMap<String, String>, String> {
    let trimmed = value.trim();
    if !trimmed.ends_with('}') {
        return Err("inline table must end with `}`".to_string());
    }
    let inner = trimmed
        .strip_prefix('{')
        .ok_or_else(|| "inline table must start with `{`".to_string())?
        .strip_suffix('}')
        .unwrap()
        .trim();
    let mut out = HashMap::new();
    if inner.is_empty() {
        return Ok(out);
    }
    for part in split_top_level(inner, ',') {
        let (key, value) = split_key_value(part.trim())
            .ok_or_else(|| "inline table item must use `key = value`".to_string())?;
        out.insert(normalize_key(key)?, parse_string_value(value)?);
    }
    Ok(out)
}

fn required_field(
    package: &HashMap<String, String>,
    key: &str,
    manifest_path: &Path,
) -> Result<String, String> {
    package.get(key).cloned().ok_or_else(|| {
        format!(
            "package.manifest_missing_field: `{}` requires `[package].{key}`",
            manifest_path.display()
        )
    })
}

fn validate_package_name(name: &str) -> Result<(), &'static str> {
    if name.is_empty() {
        return Err("empty names are not allowed");
    }
    for segment in name.split('.') {
        if segment.is_empty() {
            return Err("empty namespace segments are not allowed");
        }
        let mut chars = segment.chars();
        let first = chars
            .next()
            .ok_or("empty namespace segments are not allowed")?;
        if !(first == '_' || first.is_ascii_alphabetic()) {
            return Err("each segment must start with a letter or `_`");
        }
        if !chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric()) {
            return Err("segments may contain only letters, digits, and `_`");
        }
    }
    Ok(())
}

fn validate_semver_like(version: &str) -> Result<(), &'static str> {
    let mut parts = version.split('.');
    let Some(major) = parts.next() else {
        return Err("expected `major.minor.patch`");
    };
    let Some(minor) = parts.next() else {
        return Err("expected `major.minor.patch`");
    };
    let Some(patch) = parts.next() else {
        return Err("expected `major.minor.patch`");
    };
    if parts.next().is_some() {
        return Err("expected `major.minor.patch`");
    }
    if [major, minor, patch]
        .iter()
        .any(|part| part.is_empty() || !part.chars().all(|ch| ch.is_ascii_digit()))
    {
        return Err("version parts must be decimal numbers");
    }
    Ok(())
}

fn strip_comment(line: &str) -> &str {
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == '#' && !in_string {
            return &line[..index];
        }
    }
    line
}

fn split_key_value(line: &str) -> Option<(&str, &str)> {
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in line.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == '=' && !in_string {
            return Some((&line[..index], &line[index + 1..]));
        }
    }
    None
}

fn split_top_level(value: &str, separator: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut in_string = false;
    let mut escaped = false;
    for (index, ch) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' && in_string {
            escaped = true;
            continue;
        }
        if ch == '"' {
            in_string = !in_string;
            continue;
        }
        if ch == separator && !in_string {
            parts.push(&value[start..index]);
            start = index + ch.len_utf8();
        }
    }
    parts.push(&value[start..]);
    parts
}

fn normalize_key(raw: &str) -> Result<String, String> {
    let key = raw.trim();
    if key.starts_with('"') {
        return parse_string_value(key);
    }
    if key.is_empty() {
        return Err("empty keys are not allowed".to_string());
    }
    Ok(key.to_string())
}

fn parse_string_array_value(raw: &str) -> Result<Vec<String>, String> {
    let value = raw.trim();
    if !value.starts_with('[') || !value.ends_with(']') {
        return Err("expected an array".to_string());
    }
    let inner = value[1..value.len() - 1].trim();
    if inner.is_empty() {
        return Ok(Vec::new());
    }

    let mut items = Vec::new();
    for part in split_top_level(inner, ',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        items.push(parse_string_value(part)?);
    }
    Ok(items)
}

fn parse_string_value(raw: &str) -> Result<String, String> {
    let value = raw.trim();
    if !value.starts_with('"') || !value.ends_with('"') || value.len() < 2 {
        return Err("expected a quoted string".to_string());
    }
    let inner = &value[1..value.len() - 1];
    let mut out = String::new();
    let mut chars = inner.chars();
    while let Some(ch) = chars.next() {
        if ch != '\\' {
            out.push(ch);
            continue;
        }
        let escaped = chars
            .next()
            .ok_or_else(|| "unterminated string escape".to_string())?;
        match escaped {
            '"' => out.push('"'),
            '\\' => out.push('\\'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            other => {
                return Err(format!("unsupported string escape `\\{other}`"));
            }
        }
    }
    Ok(out)
}

fn copy_package_tree(source: &Path, target: &Path) -> Result<(), String> {
    fs::create_dir_all(target).map_err(|err| {
        format!(
            "package.cache_write_failed: cannot create `{}`: {err}",
            target.display()
        )
    })?;
    copy_dir_recursive(source, target)
}

fn copy_dir_recursive(source: &Path, target: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|err| {
        format!(
            "package.source_read_failed: cannot read `{}`: {err}",
            source.display()
        )
    })? {
        let entry = entry.map_err(|err| {
            format!(
                "package.source_read_failed: cannot read entry in `{}`: {err}",
                source.display()
            )
        })?;
        let file_name = entry.file_name();
        if file_name == ".git" || file_name == "target" || file_name == CACHE_METADATA_FILE {
            continue;
        }
        let source_path = entry.path();
        let target_path = target.join(&file_name);
        let file_type = entry.file_type().map_err(|err| {
            format!(
                "package.source_read_failed: cannot inspect `{}`: {err}",
                source_path.display()
            )
        })?;
        if file_type.is_symlink() {
            return Err(format!(
                "package.symlink_unsupported: `{}` is a symlink",
                source_path.display()
            ));
        }
        if file_type.is_dir() {
            fs::create_dir_all(&target_path).map_err(|err| {
                format!(
                    "package.cache_write_failed: cannot create `{}`: {err}",
                    target_path.display()
                )
            })?;
            copy_dir_recursive(&source_path, &target_path)?;
        } else if file_type.is_file() {
            fs::copy(&source_path, &target_path).map_err(|err| {
                format!(
                    "package.cache_write_failed: cannot copy `{}` to `{}`: {err}",
                    source_path.display(),
                    target_path.display()
                )
            })?;
        }
    }
    Ok(())
}

#[derive(Debug, serde::Deserialize, serde::Serialize)]
struct CacheEntryMetadata {
    format: u32,
    source: String,
    digest: String,
}

struct ExclusiveTempDir {
    path: PathBuf,
}

impl ExclusiveTempDir {
    fn create(parent: &Path, purpose: &str) -> Result<Self, String> {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "package.temp_failed: cannot create temporary parent `{}`: {err}",
                parent.display()
            )
        })?;
        for _ in 0..128 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(".ori-{purpose}-{}-{sequence}", std::process::id()));
            match fs::create_dir(&path) {
                Ok(()) => {
                    #[cfg(unix)]
                    {
                        use std::os::unix::fs::PermissionsExt;
                        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).map_err(
                            |err| {
                                format!(
                                    "package.temp_failed: cannot secure `{}`: {err}",
                                    path.display()
                                )
                            },
                        )?;
                    }
                    return Ok(Self { path });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(err) => {
                    return Err(format!(
                        "package.temp_failed: cannot exclusively create `{}`: {err}",
                        path.display()
                    ));
                }
            }
        }
        Err(format!(
            "package.temp_failed: cannot allocate an exclusive directory under `{}`",
            parent.display()
        ))
    }
}

impl Drop for ExclusiveTempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn package_tree_digest(root: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};

    fn collect(
        root: &Path,
        current: &Path,
        files: &mut Vec<(String, PathBuf)>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(current).map_err(|err| {
            format!(
                "package.digest_failed: cannot read `{}`: {err}",
                current.display()
            )
        })? {
            let entry = entry.map_err(|err| {
                format!(
                    "package.digest_failed: cannot read entry under `{}`: {err}",
                    current.display()
                )
            })?;
            let path = entry.path();
            let name = entry.file_name();
            if name == ".git" || name == "target" || name == CACHE_METADATA_FILE {
                continue;
            }
            let file_type = entry.file_type().map_err(|err| {
                format!(
                    "package.digest_failed: cannot inspect `{}`: {err}",
                    path.display()
                )
            })?;
            if file_type.is_symlink() {
                return Err(format!(
                    "package.symlink_unsupported: `{}` is a symlink",
                    path.display()
                ));
            }
            if file_type.is_dir() {
                collect(root, &path, files)?;
            } else if file_type.is_file() {
                let relative = path.strip_prefix(root).map_err(|_| {
                    format!(
                        "package.digest_failed: `{}` is outside `{}`",
                        path.display(),
                        root.display()
                    )
                })?;
                let relative = relative.to_str().ok_or_else(|| {
                    format!(
                        "package.digest_failed: path `{}` is not valid UTF-8",
                        relative.display()
                    )
                })?;
                files.push((relative.replace('\\', "/"), path));
            }
        }
        Ok(())
    }

    let mut files = Vec::new();
    collect(root, root, &mut files)?;
    files.sort_by(|left, right| left.0.cmp(&right.0));
    let mut digest = Sha256::new();
    digest.update(b"ori-package-tree-v1\0");
    for (relative, path) in files {
        let relative = relative.as_bytes();
        digest.update((relative.len() as u64).to_le_bytes());
        digest.update(relative);
        let mut file = fs::File::open(&path).map_err(|err| {
            format!(
                "package.digest_failed: cannot open `{}`: {err}",
                path.display()
            )
        })?;
        let length = file
            .metadata()
            .map_err(|err| {
                format!(
                    "package.digest_failed: cannot stat `{}`: {err}",
                    path.display()
                )
            })?
            .len();
        digest.update(length.to_le_bytes());
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = file.read(&mut buffer).map_err(|err| {
                format!(
                    "package.digest_failed: cannot read `{}`: {err}",
                    path.display()
                )
            })?;
            if read == 0 {
                break;
            }
            digest.update(&buffer[..read]);
        }
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn cache_metadata_path(root: &Path) -> PathBuf {
    root.join(CACHE_METADATA_FILE)
}

fn read_cache_metadata(root: &Path) -> Result<CacheEntryMetadata, String> {
    let metadata_path = cache_metadata_path(root);
    let metadata: CacheEntryMetadata =
        serde_json::from_slice(&fs::read(&metadata_path).map_err(|err| {
            format!(
                "package.cache_metadata_missing: cannot read `{}`: {err}; refetch the package",
                metadata_path.display()
            )
        })?)
        .map_err(|err| {
            format!(
                "package.cache_metadata_invalid: cannot parse `{}`: {err}",
                metadata_path.display()
            )
        })?;
    if metadata.format != 1 {
        return Err(format!(
            "package.cache_metadata_invalid: `{}` uses unsupported format {}",
            metadata_path.display(),
            metadata.format
        ));
    }
    Ok(metadata)
}

fn validate_cache_entry(
    root: &Path,
    name: &str,
    version: &str,
    expected_source: Option<&str>,
    expected_digest: Option<&str>,
) -> Result<String, String> {
    let manifest = load_package_manifest(root)?;
    if manifest.name != name || manifest.version != version {
        return Err(format!(
            "package.cache_conflict: cache entry `{}` declares `{}@{}` instead of `{name}@{version}`",
            root.display(),
            manifest.name,
            manifest.version
        ));
    }
    let metadata = read_cache_metadata(root)?;
    if let Some(expected_source) = expected_source {
        if metadata.source != expected_source {
            return Err(format!(
                "package.cache_source_mismatch: `{}` belongs to `{}`, not `{expected_source}`",
                root.display(),
                metadata.source
            ));
        }
    }
    let actual = package_tree_digest(root)?;
    if metadata.digest != actual {
        return Err(format!(
            "package.cache_digest_mismatch: cache entry `{}` changed (expected {}, got {actual})",
            root.display(),
            metadata.digest
        ));
    }
    if let Some(expected_digest) = expected_digest {
        if !actual.eq_ignore_ascii_case(expected_digest) {
            return Err(format!(
                "package.lock_digest_mismatch: cache entry `{}` hashed to {actual}, lock requires {expected_digest}",
                root.display()
            ));
        }
    }
    Ok(actual)
}

fn publish_cache_entry(
    source: &Path,
    target: &Path,
    source_identity: &str,
    expected_digest: Option<&str>,
) -> Result<String, String> {
    let manifest = load_package_manifest(source)?;
    let digest = package_tree_digest(source)?;
    if let Some(expected_digest) = expected_digest {
        if !digest.eq_ignore_ascii_case(expected_digest) {
            return Err(format!(
                "package.lock_digest_mismatch: source `{}` hashed to {digest}, lock requires {expected_digest}",
                source.display()
            ));
        }
    }
    if target.join("ori.pkg.toml").is_file() {
        return validate_cache_entry(
            target,
            &manifest.name,
            &manifest.version,
            Some(source_identity),
            Some(&digest),
        );
    }
    if target.exists() {
        return Err(format!(
            "package.cache_conflict: incomplete cache path `{}` already exists",
            target.display()
        ));
    }
    let parent = target.parent().ok_or_else(|| {
        format!(
            "package.cache_write_failed: target `{}` has no parent",
            target.display()
        )
    })?;
    let temporary = ExclusiveTempDir::create(parent, "cache-stage")?;
    let staged = temporary.path.join("tree");
    copy_package_tree(source, &staged)?;
    let staged_digest = package_tree_digest(&staged)?;
    if staged_digest != digest {
        return Err(format!(
            "package.source_changed: `{}` changed while it was copied (before {digest}, after {staged_digest}); retry",
            source.display()
        ));
    }
    let metadata = CacheEntryMetadata {
        format: 1,
        source: source_identity.to_string(),
        digest: digest.clone(),
    };
    fs::write(
        cache_metadata_path(&staged),
        serde_json::to_vec_pretty(&metadata).map_err(|err| {
            format!("package.cache_write_failed: cannot encode cache metadata: {err}")
        })?,
    )
    .map_err(|err| {
        format!(
            "package.cache_write_failed: cannot write metadata under `{}`: {err}",
            staged.display()
        )
    })?;
    match fs::rename(&staged, target) {
        Ok(()) => Ok(digest),
        Err(err) if target.join("ori.pkg.toml").is_file() => validate_cache_entry(
            target,
            &manifest.name,
            &manifest.version,
            Some(source_identity),
            Some(&digest),
        )
        .map_err(|validation| {
            format!(
                "package.cache_publish_failed: atomic publish raced at `{}` ({err}); winner invalid: {validation}",
                target.display()
            )
        }),
        Err(err) => Err(format!(
            "package.cache_publish_failed: cannot atomically publish `{}`: {err}",
            target.display()
        )),
    }
}

pub fn default_package_cache_root() -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("ORI_PACKAGE_CACHE") {
        if !path.trim().is_empty() {
            return Ok(PathBuf::from(path));
        }
    }
    if let Ok(home) = std::env::var("USERPROFILE") {
        if !home.trim().is_empty() {
            return Ok(PathBuf::from(home).join(".ori").join("packages"));
        }
    }
    if let Ok(home) = std::env::var("HOME") {
        if !home.trim().is_empty() {
            return Ok(PathBuf::from(home).join(".ori").join("packages"));
        }
    }
    Err(
        "package.cache_home_missing: set ORI_PACKAGE_CACHE to choose a local package cache"
            .to_string(),
    )
}

/// Fetch a git dependency into the cache and return the package root (`name/version`).
pub fn ensure_git_dependency_cached(
    expected_name: &str,
    spec: &GitDependencySpec,
    cache_root: &Path,
) -> Result<PathBuf, String> {
    ensure_git_dependency_cached_with_lock(expected_name, spec, cache_root, None, None, false)
}

fn ensure_git_dependency_cached_with_lock(
    expected_name: &str,
    spec: &GitDependencySpec,
    cache_root: &Path,
    locked_revision: Option<&str>,
    expected_digest: Option<&str>,
    offline: bool,
) -> Result<PathBuf, String> {
    let normalized_url = normalize_git_url(&spec.url);
    if let (Some(version), Some(revision)) = (&spec.version, locked_revision) {
        let target = cache_root.join(expected_name).join(version);
        let source_identity = git_source_identity(&normalized_url, revision);
        if target.join("ori.pkg.toml").is_file() {
            validate_cache_entry(
                &target,
                expected_name,
                version,
                Some(&source_identity),
                expected_digest,
            )?;
            return Ok(target);
        }
    }
    if offline {
        return Err(format!(
            "package.offline_cache_miss: locked Git dependency `{expected_name}` is not available in the verified cache"
        ));
    }

    let exact_spec = locked_revision.map_or_else(
        || spec.clone(),
        |revision| GitDependencySpec {
            url: spec.url.clone(),
            rev: Some(revision.to_string()),
            tag: None,
            branch: None,
            version: spec.version.clone(),
        },
    );
    let git_checkout = fetch_git_checkout(&exact_spec, cache_root)?;
    let package_root = package_root_from_path(&git_checkout)?;
    let manifest = load_package_manifest(&package_root)?;
    if manifest.name != expected_name {
        return Err(format!(
            "package.dependency_name_mismatch: dependency `{expected_name}` git source declares package `{}`",
            manifest.name
        ));
    }
    if let Some(expected_version) = &spec.version {
        if &manifest.version != expected_version {
            return Err(format!(
                "package.dependency_version_mismatch: dependency `{expected_name}` expected `{expected_version}`, found `{}`",
                manifest.version
            ));
        }
    }

    let revision = git_revision(&package_root)?;
    if let Some(expected_revision) = locked_revision {
        if revision != expected_revision {
            return Err(format!(
                "package.git_revision_mismatch: `{expected_name}` resolved to {revision}, lock requires {expected_revision}"
            ));
        }
    }
    let source_identity = git_source_identity(&normalized_url, &revision);
    let target_root = cache_root.join(&manifest.name).join(&manifest.version);
    if target_root.join("ori.pkg.toml").is_file() {
        validate_cache_entry(
            &target_root,
            &manifest.name,
            &manifest.version,
            Some(&source_identity),
            expected_digest,
        )?;
        return Ok(target_root);
    }

    publish_cache_entry(
        &manifest.root,
        &target_root,
        &source_identity,
        expected_digest,
    )?;
    Ok(target_root)
}

fn git_source_identity(url: &str, revision: &str) -> String {
    format!("git:{}#{revision}", url.trim_end_matches('/'))
}

/// Resolve a version-pinned dependency from the local cache, fetching from the
/// configured registry on cache miss when possible.
pub fn resolve_cached_version_package(
    name: &str,
    version: &str,
    cache_root: &Path,
) -> Result<PathBuf, String> {
    ensure_version_package_cached(
        name,
        version,
        cache_root,
        resolve_registry_location(None).ok().as_ref(),
    )
}

/// Ensure `name@version` is materialised under the package cache.
pub fn ensure_version_package_cached(
    name: &str,
    version: &str,
    cache_root: &Path,
    registry: Option<&RegistryLocation>,
) -> Result<PathBuf, String> {
    let root = cache_root.join(name).join(version);
    let manifest_path = root.join("ori.pkg.toml");
    if manifest_path.is_file() {
        let expected_source = registry
            .map(RegistryLocation::source_identity)
            .transpose()?;
        validate_cache_entry(&root, name, version, expected_source.as_deref(), None)?;
        return Ok(root);
    }

    if env_flag(OFFLINE_ENV) {
        return Err(format!(
            "package.offline_cache_miss: `{name}@{version}` is not available in the verified cache at `{}`",
            root.display()
        ));
    }
    if let Some(registry) = registry {
        return fetch_package_from_registry(name, version, registry, cache_root, None);
    }

    Err(format!(
        "package.cache_miss: dependency `{name}` version `{version}` is not in cache at `{}` and no registry is configured (set ORI_REGISTRY, use `path`/`git`, or `ori install --path`)",
        root.display()
    ))
}

/// Fetch all git (and install path) dependencies declared by a project or package manifest.
pub fn run_get_dependencies(
    options: GetDependenciesOptions,
) -> Result<GetDependenciesOutput, String> {
    let cache_root = match options.cache_root {
        Some(path) => path,
        None => default_package_cache_root()?,
    };
    let root = package_root_from_path(&options.path)?;
    let existing_lock = package_lock_path(&root);
    if existing_lock.is_file() {
        let lock = load_package_lock(&existing_lock)?;
        let manifest = load_lock_manifest(&root)?;
        validate_locked_manifest(&manifest, &lock)?;
        restore_locked_dependencies(&root, &lock, Some(&cache_root), env_flag(OFFLINE_ENV))?;
        let packages = locked_packages_output(&root, &cache_root, &lock)?;
        return Ok(GetDependenciesOutput {
            cache_root,
            packages,
        });
    }
    let mut seen = HashSet::new();
    let mut packages = Vec::new();

    let pkg_manifest = root.join("ori.pkg.toml");
    let proj_manifest = root.join("ori.proj");

    if pkg_manifest.is_file() {
        install_local_package(&root, None, &cache_root, &mut seen, &mut packages)?;
        run_lock_package(LockPackageOptions {
            path: root.clone(),
            locked: false,
            cache_root: Some(cache_root.clone()),
            offline: false,
        })?;
    } else if proj_manifest.is_file() {
        fetch_project_git_dependencies(&proj_manifest, &cache_root, &mut seen, &mut packages)?;
        run_lock_package(LockPackageOptions {
            path: root.clone(),
            locked: false,
            cache_root: Some(cache_root.clone()),
            offline: false,
        })?;
    } else if options.path.is_file() {
        let name = options
            .path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        if name == "ori.pkg.toml" {
            install_local_package(
                &package_root_from_path(&options.path)?,
                None,
                &cache_root,
                &mut seen,
                &mut packages,
            )?;
        } else if name == "ori.proj" {
            fetch_project_git_dependencies(&options.path, &cache_root, &mut seen, &mut packages)?;
            if let Some(project_root) = options.path.parent() {
                run_lock_package(LockPackageOptions {
                    path: project_root.to_path_buf(),
                    locked: false,
                    cache_root: Some(cache_root.clone()),
                    offline: false,
                })?;
            }
        } else {
            return Err(format!(
                "package.get_target_invalid: `{}` is not an Ori project/package root",
                options.path.display()
            ));
        }
    } else {
        return Err(format!(
            "package.get_target_invalid: `{}` has neither `ori.proj` nor `ori.pkg.toml`",
            root.display()
        ));
    }

    Ok(GetDependenciesOutput {
        cache_root,
        packages,
    })
}

fn locked_packages_output(
    project_root: &Path,
    cache_root: &Path,
    lock: &PackageLockfile,
) -> Result<Vec<InstalledPackage>, String> {
    lock.dependencies
        .iter()
        .map(|dependency| {
            let installed_root = match dependency.source {
                LockedDependencySource::Path => dependency
                    .url
                    .as_deref()
                    .and_then(|source| source.strip_prefix("path:"))
                    .map(PathBuf::from)
                    .unwrap_or_else(|| {
                        project_root.join(dependency.path.as_deref().unwrap_or_default())
                    }),
                LockedDependencySource::Registry | LockedDependencySource::Git => {
                    cache_root.join(&dependency.name).join(&dependency.version)
                }
            };
            Ok(InstalledPackage {
                name: dependency.name.clone(),
                version: dependency.version.clone(),
                source_root: installed_root.clone(),
                installed_root,
                already_installed: true,
            })
        })
        .collect()
}

fn fetch_project_git_dependencies(
    proj_manifest: &Path,
    cache_root: &Path,
    seen: &mut HashSet<String>,
    packages: &mut Vec<InstalledPackage>,
) -> Result<(), String> {
    let source = fs::read_to_string(proj_manifest).map_err(|err| {
        format!(
            "package.manifest_missing: cannot read `{}`: {err}",
            proj_manifest.display()
        )
    })?;
    let root = proj_manifest
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| {
            format!(
                "package.manifest_invalid: `{}` has no parent",
                proj_manifest.display()
            )
        })?;

    let mut in_dependencies = false;
    for raw_line in source.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if line.starts_with('[') && line.ends_with(']') {
            in_dependencies = &line[1..line.len() - 1] == "dependencies";
            continue;
        }
        if !in_dependencies {
            continue;
        }
        let (key, value) = split_key_value(line).ok_or_else(|| {
            format!(
                "package.manifest_syntax: `{}` expected `name = ...` in [dependencies]",
                proj_manifest.display()
            )
        })?;
        let dep = parse_dependency(key, value)?;
        match dep.requirement {
            DependencyRequirement::Git {
                url,
                rev,
                tag,
                branch,
                version,
            } => {
                let spec = GitDependencySpec::from_requirement(url, rev, tag, branch, version);
                let installed_root = ensure_git_dependency_cached(&dep.name, &spec, cache_root)?;
                let manifest = load_package_manifest(&installed_root)?;
                let key = format!("{}@{}", manifest.name, manifest.version);
                if seen.insert(key) {
                    packages.push(InstalledPackage {
                        name: manifest.name,
                        version: manifest.version,
                        source_root: installed_root.clone(),
                        installed_root,
                        already_installed: true,
                    });
                }
            }
            DependencyRequirement::Path { path, version } => {
                let dependency_root = root.join(path);
                if let Some(expected) = &version {
                    let m = load_package_manifest(&dependency_root)?;
                    if m.version != *expected {
                        return Err(format!(
                            "package.dependency_version_mismatch: dependency `{}` expected `{expected}`, found `{}`",
                            dep.name, m.version
                        ));
                    }
                }
                install_local_package(
                    &dependency_root,
                    Some(&dep.name),
                    cache_root,
                    seen,
                    packages,
                )?;
            }
            DependencyRequirement::Version(version) => {
                let root = resolve_cached_version_package(&dep.name, &version, cache_root)?;
                install_local_package(&root, Some(&dep.name), cache_root, seen, packages)?;
            }
        }
    }
    Ok(())
}

fn fetch_git_checkout(spec: &GitDependencySpec, cache_root: &Path) -> Result<PathBuf, String> {
    let url = normalize_git_url(&spec.url);
    let url_key = stable_source_key(&url);
    let ref_key = spec.ref_key();
    let checkout_root = cache_root.join("git").join(url_key).join(ref_key);

    if checkout_root.join("ori.pkg.toml").is_file()
        || checkout_root.join("ori.proj").is_file()
        || has_nested_package_manifest(&checkout_root)
    {
        return find_package_root_in_checkout(&checkout_root);
    }

    if checkout_root.exists() {
        return Err(format!(
            "package.cache_conflict: incomplete Git checkout `{}` already exists",
            checkout_root.display()
        ));
    }
    let parent = checkout_root.parent().ok_or_else(|| {
        format!(
            "package.cache_write_failed: checkout `{}` has no parent",
            checkout_root.display()
        )
    })?;
    let temporary = ExclusiveTempDir::create(parent, "git-stage")?;
    let staged_checkout = temporary.path.join("checkout");
    eprintln!(
        "ori: fetching git dependency {} ({})...",
        url,
        spec.ref_key()
    );

    let mut clone = Command::new("git");
    clone.arg("clone");
    if spec.rev.is_none() {
        clone.arg("--depth").arg("1");
        if let Some(tag) = &spec.tag {
            clone.arg("--branch").arg(tag);
        } else if let Some(branch) = &spec.branch {
            clone.arg("--branch").arg(branch);
        } else {
            // default branch when unpinned — try main, fall back to remote default
            clone.arg("--branch").arg("main");
        }
    }
    clone.arg(&url).arg(&staged_checkout);

    let status = clone
        .status()
        .map_err(|err| format!("package.git_clone_failed: failed to invoke git: {err}"))?;

    if !status.success() {
        // Retry without --branch main if default failed (remote may use master)
        if spec.rev.is_none() && spec.tag.is_none() && spec.branch.is_none() {
            fs::remove_dir_all(&staged_checkout).map_err(|err| {
                format!(
                    "package.git_clone_failed: cannot clear failed checkout `{}`: {err}",
                    staged_checkout.display()
                )
            })?;
            let status = Command::new("git")
                .arg("clone")
                .arg("--depth")
                .arg("1")
                .arg(&url)
                .arg(&staged_checkout)
                .status()
                .map_err(|err| format!("package.git_clone_failed: failed to invoke git: {err}"))?;
            if !status.success() {
                return Err(format!(
                    "package.git_clone_failed: git clone failed for `{url}` (status {status})"
                ));
            }
        } else {
            return Err(format!(
                "package.git_clone_failed: git clone failed for `{url}` (status {status})"
            ));
        }
    }

    if let Some(rev) = &spec.rev {
        let status = Command::new("git")
            .arg("-C")
            .arg(&staged_checkout)
            .arg("checkout")
            .arg(rev)
            .status()
            .map_err(|err| format!("package.git_checkout_failed: failed to invoke git: {err}"))?;
        if !status.success() {
            return Err(format!(
                "package.git_checkout_failed: cannot checkout rev `{rev}` in `{}`",
                staged_checkout.display()
            ));
        }
    }
    find_package_root_in_checkout(&staged_checkout)?;
    match fs::rename(&staged_checkout, &checkout_root) {
        Ok(()) => {}
        Err(_) if checkout_root.exists() => {
            find_package_root_in_checkout(&checkout_root)?;
        }
        Err(err) => {
            return Err(format!(
                "package.cache_publish_failed: cannot atomically publish Git checkout `{}`: {err}",
                checkout_root.display()
            ));
        }
    }
    find_package_root_in_checkout(&checkout_root)
}

fn stable_source_key(source: &str) -> String {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(source.as_bytes());
    format!("{:x}", digest)
}

fn has_nested_package_manifest(root: &Path) -> bool {
    root.join("ori.pkg.toml").is_file() || root.join("ori.proj").is_file()
}

fn find_package_root_in_checkout(checkout_root: &Path) -> Result<PathBuf, String> {
    if checkout_root.join("ori.pkg.toml").is_file() {
        return Ok(checkout_root.to_path_buf());
    }
    // Allow a single top-level subdirectory containing the package (common monorepo layout is out of scope).
    if let Ok(entries) = fs::read_dir(checkout_root) {
        let mut candidates = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && path.join("ori.pkg.toml").is_file() {
                candidates.push(path);
            }
        }
        if candidates.len() == 1 {
            return Ok(candidates.remove(0));
        }
    }
    Err(format!(
        "package.git_manifest_missing: clone at `{}` has no `ori.pkg.toml` (place it at the repo root)",
        checkout_root.display()
    ))
}

fn normalize_git_url(url: &str) -> String {
    if url.starts_with("github.com/") {
        return format!("https://{url}");
    }
    if url.starts_with("file://")
        || url.starts_with("http://")
        || url.starts_with("https://")
        || url.starts_with("git@")
    {
        return url.to_string();
    }
    // bare local path
    if Path::new(url).exists() {
        return url.to_string();
    }
    url.to_string()
}

fn sanitize_cache_segment(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' || ch == '.' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "x".to_string()
    } else {
        out
    }
}

// ---------------------------------------------------------------------------
// PKG-3 — registry (filesystem + optional HTTP tarball)
// ---------------------------------------------------------------------------

/// Where packages are published to / fetched from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RegistryLocation {
    /// Directory layout: `{root}/packages/{name}/{version}/…` + `versions.json`.
    Path(PathBuf),
    /// HTTP(S) base URL (no trailing slash). Fetch uses
    /// `{base}/packages/{name}/{version}.tar.gz`; publish uses HTTP PUT of the same.
    Http(String),
}

impl RegistryLocation {
    fn source_identity(&self) -> Result<String, String> {
        match self {
            Self::Path(path) => Ok(format!("registry:{}", path_source_identity(path)?)),
            Self::Http(base) => Ok(format!("registry:{}", base.trim_end_matches('/'))),
        }
    }

    fn from_source_identity(identity: &str) -> Result<Self, String> {
        let raw = identity.strip_prefix("registry:").ok_or_else(|| {
            format!("package.lock_source_invalid: `{identity}` is not a registry identity")
        })?;
        if let Some(path) = raw.strip_prefix("path:") {
            return Ok(Self::Path(PathBuf::from(path)));
        }
        if raw.starts_with("https://") || raw.starts_with("http://") {
            validate_registry_transport(raw)?;
            return Ok(Self::Http(raw.trim_end_matches('/').to_string()));
        }
        Err(format!(
            "package.lock_source_invalid: unsupported registry identity `{identity}`"
        ))
    }
}

#[derive(Debug, Clone)]
pub struct PublishPackageOptions {
    pub path: PathBuf,
    /// Override `ORI_REGISTRY`.
    pub registry: Option<String>,
    /// Override `ORI_REGISTRY_TOKEN` (HTTP publish Authorization Bearer).
    pub token: Option<String>,
    /// Replace an existing `name@version` in the registry.
    pub force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishPackageOutput {
    pub name: String,
    pub version: String,
    pub registry: String,
    pub location: String,
}

/// Publish a validated package into the configured registry.
pub fn run_publish_package(options: PublishPackageOptions) -> Result<PublishPackageOutput, String> {
    let root = package_root_from_path(&options.path)?;
    let manifest = load_package_manifest(&root)?;
    // Reject path-only publish of packages that cannot stand alone? Allow all.

    let registry = resolve_registry_location(options.registry.as_deref())?;
    let token = options
        .token
        .or_else(|| std::env::var("ORI_REGISTRY_TOKEN").ok())
        .filter(|t| !t.trim().is_empty());
    if options.force {
        return Err(
            "package.publish_immutable: published package versions are immutable; bump the version instead of using --force"
                .to_string(),
        );
    }

    match &registry {
        RegistryLocation::Path(reg_root) => {
            let _publish_lock = RegistryPublishLock::acquire(reg_root)?;
            let dest = reg_root
                .join("packages")
                .join(&manifest.name)
                .join(&manifest.version);
            if dest.exists() {
                return Err(format!(
                    "package.publish_exists: `{}@{}` already published at `{}`; package versions are immutable",
                    manifest.name,
                    manifest.version,
                    dest.display()
                ));
            }
            let package_dir = reg_root.join("packages").join(&manifest.name);
            let temporary = ExclusiveTempDir::create(&package_dir, "publish")?;
            let staged_tree = temporary.path.join("tree");
            copy_package_tree(&manifest.root, &staged_tree)?;
            let staged_tarball = temporary.path.join("package.tar.gz");
            create_package_tarball(&manifest.root, &staged_tarball)?;
            let archive_digest = sha256_file(&staged_tarball)?;
            let staged_digest = temporary.path.join("package.tar.gz.sha256");
            fs::write(&staged_digest, format!("{archive_digest}\n")).map_err(|err| {
                format!(
                    "package.publish_failed: cannot write `{}`: {err}",
                    staged_digest.display()
                )
            })?;
            let tarball = package_dir.join(format!("{}.tar.gz", manifest.version));
            let digest_file = package_dir.join(format!("{}.tar.gz.sha256", manifest.version));
            if tarball.exists() || digest_file.exists() {
                return Err(format!(
                    "package.publish_exists: archive for `{}@{}` already exists; package versions are immutable",
                    manifest.name, manifest.version
                ));
            }
            fs::rename(&staged_tarball, &tarball).map_err(|err| {
                format!(
                    "package.publish_failed: cannot publish archive `{}`: {err}",
                    tarball.display()
                )
            })?;
            if let Err(err) = fs::rename(&staged_digest, &digest_file) {
                let _ = fs::remove_file(&tarball);
                return Err(format!(
                    "package.publish_failed: archive digest publication failed and the archive was rolled back: {err}"
                ));
            }
            if let Err(err) = fs::rename(&staged_tree, &dest) {
                let _ = fs::remove_file(&digest_file);
                let _ = fs::remove_file(&tarball);
                return Err(format!(
                    "package.publish_failed: package tree publication failed and staged artifacts were rolled back: {err}"
                ));
            }
            update_versions_index(reg_root, &manifest.name, &manifest.version)?;
            update_global_index(reg_root, &manifest.name, &manifest.version)?;

            Ok(PublishPackageOutput {
                name: manifest.name,
                version: manifest.version,
                registry: reg_root.display().to_string(),
                location: dest.display().to_string(),
            })
        }
        RegistryLocation::Http(base) => {
            let tarball_url = format!(
                "{}/packages/{}/{}.tar.gz",
                base.trim_end_matches('/'),
                manifest.name,
                manifest.version
            );
            let temporary = ExclusiveTempDir::create(&std::env::temp_dir(), "registry-publish")?;
            let archive = temporary.path.join("package.tar.gz");
            create_package_tarball(&manifest.root, &archive)?;
            let digest = sha256_file(&archive)?;
            let digest_file = temporary.path.join("package.tar.gz.sha256");
            fs::write(&digest_file, format!("{digest}\n")).map_err(|err| {
                format!(
                    "package.publish_failed: cannot write `{}`: {err}",
                    digest_file.display()
                )
            })?;
            // Publish the digest first. Fetchers require it before the archive,
            // so a concurrent reader sees either a verified package or a safe
            // cache miss, never an unverified archive.
            http_put_file(
                &format!("{tarball_url}.sha256"),
                &digest_file,
                token.as_deref(),
            )?;
            http_put_file(&tarball_url, &archive, token.as_deref())?;

            // Best-effort: upload versions.json merge is not possible without GET+PUT race;
            // clients can list via the tarball convention. Document that file registries
            // own the index; HTTP registries may only host tarballs.
            Ok(PublishPackageOutput {
                name: manifest.name,
                version: manifest.version,
                registry: base.clone(),
                location: tarball_url,
            })
        }
    }
}

pub fn resolve_registry_location(override_url: Option<&str>) -> Result<RegistryLocation, String> {
    let raw = override_url
        .map(|s| s.to_string())
        .or_else(|| std::env::var("ORI_REGISTRY").ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            "package.registry_unconfigured: set ORI_REGISTRY to a directory path or http(s) base URL (e.g. /var/ori-registry or https://registry.example/ori)"
                .to_string()
        })?;

    if raw.starts_with("http://") || raw.starts_with("https://") {
        validate_registry_transport(&raw)?;
        return Ok(RegistryLocation::Http(
            raw.trim_end_matches('/').to_string(),
        ));
    }
    let path = if let Some(rest) = raw.strip_prefix("file://") {
        PathBuf::from(rest)
    } else {
        PathBuf::from(&raw)
    };
    Ok(RegistryLocation::Path(path))
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "on"
        )
    })
}

fn validate_registry_transport(url: &str) -> Result<(), String> {
    if url.starts_with("https://") {
        return Ok(());
    }
    if url.starts_with("http://") && env_flag(INSECURE_REGISTRY_ENV) {
        return Ok(());
    }
    Err(format!(
        "package.insecure_registry: `{url}` is not HTTPS; set {INSECURE_REGISTRY_ENV}=1 only for an explicitly trusted local development registry"
    ))
}

fn fetch_package_from_registry(
    name: &str,
    version: &str,
    registry: &RegistryLocation,
    cache_root: &Path,
    expected_tree_digest: Option<&str>,
) -> Result<PathBuf, String> {
    let target = cache_root.join(name).join(version);
    let source_identity = registry.source_identity()?;
    match registry {
        RegistryLocation::Path(reg_root) => {
            let source = reg_root.join("packages").join(name).join(version);
            if !source.join("ori.pkg.toml").is_file() {
                return Err(format!(
                    "package.registry_miss: `{name}@{version}` not found in registry at `{}`",
                    source.display()
                ));
            }
            let manifest = load_package_manifest(&source)?;
            if manifest.name != name || manifest.version != version {
                return Err(format!(
                    "package.registry_conflict: registry entry `{}` declares `{}@{}`",
                    source.display(),
                    manifest.name,
                    manifest.version
                ));
            }
            eprintln!(
                "ori: fetching `{name}@{version}` from registry {}...",
                reg_root.display()
            );
            publish_cache_entry(&source, &target, &source_identity, expected_tree_digest)?;
            Ok(target)
        }
        RegistryLocation::Http(base) => {
            let archive_url = format!(
                "{}/packages/{}/{}.tar.gz",
                base.trim_end_matches('/'),
                name,
                version
            );
            let digest_url = format!("{archive_url}.sha256");
            eprintln!("ori: fetching `{name}@{version}` from {archive_url}...");
            let temporary = ExclusiveTempDir::create(cache_root, "registry-fetch")?;
            let archive = temporary.path.join("package.tar.gz");
            let digest_path = temporary.path.join("package.tar.gz.sha256");
            http_get_file(&digest_url, &digest_path, MAX_REGISTRY_METADATA_BYTES)?;
            let expected_archive_digest = parse_sha256_file(&digest_path)?;
            http_get_file(&archive_url, &archive, MAX_PACKAGE_DOWNLOAD_BYTES)?;
            verify_file_sha256(&archive, &expected_archive_digest)?;
            let extract_root = temporary.path.join("extracted");
            fs::create_dir(&extract_root).map_err(|err| {
                format!(
                    "package.cache_write_failed: cannot create `{}`: {err}",
                    extract_root.display()
                )
            })?;
            extract_package_tarball(&archive, &extract_root)?;
            let package_root = find_package_root_in_checkout(&extract_root).or_else(|_| {
                // tarball may contain files at top level
                if extract_root.join("ori.pkg.toml").is_file() {
                    Ok(extract_root.clone())
                } else {
                    Err(format!(
                        "package.registry_invalid: tarball for `{name}@{version}` has no ori.pkg.toml"
                    ))
                }
            })?;
            let manifest = load_package_manifest(&package_root)?;
            if manifest.name != name || manifest.version != version {
                return Err(format!(
                    "package.registry_conflict: tarball declares `{}@{}`",
                    manifest.name, manifest.version
                ));
            }
            publish_cache_entry(
                &package_root,
                &target,
                &source_identity,
                expected_tree_digest,
            )?;
            Ok(target)
        }
    }
}

fn latest_registry_version(registry: &RegistryLocation, name: &str) -> Result<String, String> {
    match registry {
        RegistryLocation::Path(reg_root) => {
            let versions_path = reg_root.join("packages").join(name).join("versions.json");
            if versions_path.is_file() {
                let text = fs::read_to_string(&versions_path).map_err(|err| {
                    format!(
                        "package.registry_read_failed: cannot read `{}`: {err}",
                        versions_path.display()
                    )
                })?;
                let versions = parse_versions_json(&text)?;
                return versions.last().cloned().ok_or_else(|| {
                    format!("package.registry_miss: package `{name}` has no versions in registry")
                });
            }
            // Fall back: scan version directories
            let dir = reg_root.join("packages").join(name);
            let mut versions = list_version_dirs(&dir)?;
            versions.sort_by(|a, b| compare_semver_like(a, b));
            versions.last().cloned().ok_or_else(|| {
                format!(
                    "package.registry_miss: package `{name}` not found under `{}`",
                    dir.display()
                )
            })
        }
        RegistryLocation::Http(base) => {
            let url = format!(
                "{}/packages/{}/versions.json",
                base.trim_end_matches('/'),
                name
            );
            let temporary = ExclusiveTempDir::create(&std::env::temp_dir(), "registry-index")?;
            let index_path = temporary.path.join("versions.json");
            http_get_file(&url, &index_path, MAX_REGISTRY_METADATA_BYTES).map_err(|err| {
                format!(
                    "package.registry_miss: cannot read versions for `{name}` from `{url}`: {err}"
                )
            })?;
            let text = fs::read_to_string(&index_path).map_err(|err| {
                format!("package.registry_read_failed: cannot read versions file: {err}")
            })?;
            let versions = parse_versions_json(&text)?;
            versions.last().cloned().ok_or_else(|| {
                format!("package.registry_miss: package `{name}` has empty versions.json")
            })
        }
    }
}

fn update_versions_index(reg_root: &Path, name: &str, version: &str) -> Result<(), String> {
    let dir = reg_root.join("packages").join(name);
    fs::create_dir_all(&dir).map_err(|err| {
        format!(
            "package.publish_failed: cannot create `{}`: {err}",
            dir.display()
        )
    })?;
    let path = dir.join("versions.json");
    let mut versions = if path.is_file() {
        let text = fs::read_to_string(&path).map_err(|err| {
            format!(
                "package.publish_failed: cannot read `{}`: {err}",
                path.display()
            )
        })?;
        parse_versions_json(&text).map_err(|err| {
            format!(
                "package.publish_failed: invalid `{}`: {err}",
                path.display()
            )
        })?
    } else {
        Vec::new()
    };
    if !versions.iter().any(|v| v == version) {
        versions.push(version.to_string());
    }
    versions.sort_by(|a, b| compare_semver_like(a, b));
    let body = format!(
        "{{\n  \"versions\": [{}]\n}}\n",
        versions
            .iter()
            .map(|v| format!("\"{v}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    atomic_replace_registry_index(&path, body.as_bytes())
}

fn update_global_index(reg_root: &Path, name: &str, version: &str) -> Result<(), String> {
    let path = reg_root.join("index.json");
    let mut packages: HashMap<String, Vec<String>> = HashMap::new();
    if path.is_file() {
        let text = fs::read_to_string(&path).map_err(|err| {
            format!(
                "package.publish_failed: cannot read `{}`: {err}",
                path.display()
            )
        })?;
        packages = parse_global_index(&text).map_err(|err| {
            format!(
                "package.publish_failed: invalid `{}`: {err}",
                path.display()
            )
        })?;
    }
    let entry = packages.entry(name.to_string()).or_default();
    if !entry.iter().any(|v| v == version) {
        entry.push(version.to_string());
    }
    entry.sort_by(|a, b| compare_semver_like(a, b));

    let mut lines = vec!["{".to_string(), "  \"packages\": {".to_string()];
    let mut names: Vec<_> = packages.keys().cloned().collect();
    names.sort();
    for (i, n) in names.iter().enumerate() {
        let vers = &packages[n];
        let list = vers
            .iter()
            .map(|v| format!("\"{v}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let comma = if i + 1 < names.len() { "," } else { "" };
        lines.push(format!("    \"{n}\": [{list}]{comma}"));
    }
    lines.push("  }".to_string());
    lines.push("}".to_string());
    lines.push(String::new());
    let body = lines.join("\n");
    atomic_replace_registry_index(&path, body.as_bytes())
}

/// Replace a registry index through a synced temporary file. Readers see the
/// old complete document or the new complete document, never a truncation.
fn atomic_replace_registry_index(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let temporary_dir = ExclusiveTempDir::create(parent, "index-write")?;
    let temporary = temporary_dir.path.join("index.json");
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|err| {
            format!(
                "package.publish_failed: cannot exclusively create `{}`: {err}",
                temporary.display()
            )
        })?;
    file.write_all(bytes).map_err(|err| {
        format!(
            "package.publish_failed: cannot write `{}`: {err}",
            temporary.display()
        )
    })?;
    file.sync_all().map_err(|err| {
        format!(
            "package.publish_failed: cannot sync `{}`: {err}",
            temporary.display()
        )
    })?;
    drop(file);
    match fs::rename(&temporary, path) {
        Ok(()) => Ok(()),
        Err(rename_error) if cfg!(windows) && path.is_file() => fs::remove_file(path)
            .and_then(|_| fs::rename(&temporary, path))
            .map_err(|replace_error| {
                format!(
                    "package.publish_failed: cannot replace `{}`: {rename_error}; retry failed: {replace_error}",
                    path.display()
                )
            }),
        Err(err) => Err(format!(
            "package.publish_failed: cannot replace `{}`: {err}",
            path.display()
        )),
    }
}

/// Cross-process serialization for file-registry publication. The lock is
/// intentionally short-lived and covers artifact existence checks plus both
/// index updates, preventing two publishers from losing each other's version.
struct RegistryPublishLock {
    path: PathBuf,
}

impl RegistryPublishLock {
    fn acquire(reg_root: &Path) -> Result<Self, String> {
        fs::create_dir_all(reg_root).map_err(|err| {
            format!(
                "package.publish_failed: cannot create registry `{}`: {err}",
                reg_root.display()
            )
        })?;
        let path = reg_root.join(".ori-publish.lock");
        for _ in 0..200 {
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let _ = writeln!(file, "pid={}", std::process::id());
                    let _ = file.sync_all();
                    return Ok(Self { path });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    let stale = fs::metadata(&path)
                        .and_then(|metadata| metadata.modified())
                        .ok()
                        .and_then(|modified| modified.elapsed().ok())
                        .is_some_and(|age| age > Duration::from_secs(120));
                    if stale {
                        let _ = fs::remove_file(&path);
                        continue;
                    }
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(err) => {
                    return Err(format!(
                        "package.publish_failed: cannot create registry lock `{}`: {err}",
                        path.display()
                    ));
                }
            }
        }
        Err(format!(
            "package.publish_busy: registry lock `{}` remained held for two seconds",
            path.display()
        ))
    }
}

impl Drop for RegistryPublishLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

fn parse_versions_json(text: &str) -> Result<Vec<String>, String> {
    // Minimal JSON: {"versions":["0.1.0","0.2.0"]}
    let trimmed = text.trim();
    let Some(start) = trimmed.find('[') else {
        return Err("package.registry_index_invalid: versions.json missing array".into());
    };
    let Some(end) = trimmed.rfind(']') else {
        return Err("package.registry_index_invalid: versions.json missing array end".into());
    };
    let inner = &trimmed[start + 1..end];
    let mut out = Vec::new();
    for part in inner.split(',') {
        let p = part.trim().trim_matches('"').trim();
        if !p.is_empty() {
            out.push(p.to_string());
        }
    }
    Ok(out)
}

fn parse_global_index(text: &str) -> Result<HashMap<String, Vec<String>>, String> {
    // Very small parser for {"packages":{"a":["0.1.0"],"b":["1.0.0"]}}
    let mut map = HashMap::new();
    let Some(packages_pos) = text.find("\"packages\"") else {
        return Ok(map);
    };
    let rest = &text[packages_pos..];
    let Some(brace) = rest.find('{') else {
        return Ok(map);
    };
    let mut depth = 0i32;
    let mut body_start = None;
    let mut body_end = None;
    for (i, ch) in rest[brace..].char_indices() {
        match ch {
            '{' => {
                if depth == 0 {
                    body_start = Some(brace + i + 1);
                }
                depth += 1;
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    body_end = Some(brace + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let (Some(s), Some(e)) = (body_start, body_end) else {
        return Ok(map);
    };
    let body = &rest[s..e];
    // Split top-level "name": [...]
    let mut i = 0;
    let bytes = body.as_bytes();
    while i < bytes.len() {
        while i < bytes.len() && ((bytes[i] as char).is_whitespace() || bytes[i] == b',') {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        i += 1;
        let name_start = i;
        while i < bytes.len() && bytes[i] != b'"' {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let name = &body[name_start..i];
        i += 1;
        while i < bytes.len() && bytes[i] != b'[' {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let arr_start = i;
        while i < bytes.len() && bytes[i] != b']' {
            i += 1;
        }
        if i >= bytes.len() {
            break;
        }
        let arr = &body[arr_start..=i];
        let versions = parse_versions_json(&format!("{{\"versions\":{arr}}}")).unwrap_or_default();
        map.insert(name.to_string(), versions);
        i += 1;
    }
    Ok(map)
}

fn list_version_dirs(dir: &Path) -> Result<Vec<String>, String> {
    if !dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut out = Vec::new();
    for entry in fs::read_dir(dir).map_err(|err| {
        format!(
            "package.registry_read_failed: cannot read `{}`: {err}",
            dir.display()
        )
    })? {
        let entry = entry.map_err(|err| format!("package.registry_read_failed: {err}"))?;
        let path = entry.path();
        if path.is_dir() && path.join("ori.pkg.toml").is_file() {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                out.push(name.to_string());
            }
        }
    }
    Ok(out)
}

fn compare_semver_like(a: &str, b: &str) -> std::cmp::Ordering {
    let parse = |s: &str| -> (u64, u64, u64) {
        let mut parts = s.split('.');
        let maj = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let min = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        let pat = parts.next().and_then(|p| p.parse().ok()).unwrap_or(0);
        (maj, min, pat)
    };
    parse(a).cmp(&parse(b))
}

fn split_name_version(spec: &str) -> (&str, Option<&str>) {
    if let Some((name, ver)) = spec.rsplit_once('@') {
        if !name.is_empty()
            && !ver.is_empty()
            && ver.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            return (name, Some(ver));
        }
    }
    (spec, None)
}

fn create_package_tarball(source_root: &Path, tarball: &Path) -> Result<(), String> {
    let parent = tarball.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|err| {
        format!(
            "package.publish_failed: cannot create `{}`: {err}",
            parent.display()
        )
    })?;

    // Build from a sanitized copy. This keeps VCS data, compiler output,
    // cache metadata, and every symlink out of the published archive.
    let sanitized = ExclusiveTempDir::create(parent, "archive-source")?;
    let sanitized_tree = sanitized.path.join("tree");
    copy_package_tree(source_root, &sanitized_tree)?;
    package_tree_digest(&sanitized_tree)?;

    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(tarball)
        .map_err(|err| {
            format!(
                "package.tarball_failed: cannot exclusively create `{}`: {err}",
                tarball.display()
            )
        })?;
    let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
    let mut builder = tar::Builder::new(encoder);
    builder.follow_symlinks(false);
    builder
        .append_dir_all(".", &sanitized_tree)
        .map_err(|err| {
            format!(
                "package.tarball_failed: cannot archive `{}`: {err}",
                source_root.display()
            )
        })?;
    let encoder = builder
        .into_inner()
        .map_err(|err| format!("package.tarball_failed: cannot finish tar stream: {err}"))?;
    let output = encoder
        .finish()
        .map_err(|err| format!("package.tarball_failed: cannot finish gzip stream: {err}"))?;
    output
        .sync_all()
        .map_err(|err| format!("package.tarball_failed: cannot sync archive: {err}"))?;
    validate_package_tarball(tarball)
}

fn extract_package_tarball(tarball: &Path, dest: &Path) -> Result<(), String> {
    validate_package_tarball(tarball)?;
    let file = fs::File::open(tarball).map_err(|err| {
        format!(
            "package.tarball_failed: cannot open `{}`: {err}",
            tarball.display()
        )
    })?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    archive.set_overwrite(false);
    archive.set_unpack_xattrs(false);
    archive.set_preserve_permissions(false);
    archive.set_preserve_ownerships(false);
    archive.set_preserve_mtime(false);
    let entries = archive.entries().map_err(|err| {
        format!(
            "package.tarball_invalid: cannot read `{}`: {err}",
            tarball.display()
        )
    })?;
    for entry in entries {
        let mut entry = entry.map_err(|err| {
            format!(
                "package.tarball_invalid: cannot read entry from `{}`: {err}",
                tarball.display()
            )
        })?;
        if !entry.unpack_in(dest).map_err(|err| {
            format!(
                "package.tarball_invalid: cannot safely extract into `{}`: {err}",
                dest.display()
            )
        })? {
            return Err(format!(
                "package.tarball_invalid: archive member escapes `{}`",
                dest.display()
            ));
        }
    }
    Ok(())
}

const MAX_PACKAGE_ARCHIVE_ENTRIES: usize = 10_000;
const MAX_PACKAGE_ARCHIVE_PATH_BYTES: usize = 4096;

/// Validate paths, types, entry count, and expanded bytes before materializing
/// any archive-controlled filesystem object.
fn validate_package_tarball(tarball: &Path) -> Result<(), String> {
    validate_package_tarball_with_limit(tarball, MAX_PACKAGE_EXPANDED_BYTES)
}

fn validate_package_tarball_with_limit(
    tarball: &Path,
    max_expanded_bytes: u64,
) -> Result<(), String> {
    let compressed_bytes = fs::metadata(tarball)
        .map_err(|err| {
            format!(
                "package.tarball_failed: cannot stat `{}`: {err}",
                tarball.display()
            )
        })?
        .len();
    if compressed_bytes > MAX_PACKAGE_DOWNLOAD_BYTES {
        return Err(format!(
            "package.download_limit: archive `{}` is {compressed_bytes} bytes; limit is {MAX_PACKAGE_DOWNLOAD_BYTES}",
            tarball.display()
        ));
    }
    let file = fs::File::open(tarball).map_err(|err| {
        format!(
            "package.tarball_failed: cannot open `{}`: {err}",
            tarball.display()
        )
    })?;
    let decoder = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decoder);
    let entries = archive.entries().map_err(|err| {
        format!(
            "package.tarball_invalid: cannot read `{}`: {err}",
            tarball.display()
        )
    })?;
    let mut seen = HashSet::new();
    let mut seen_case_folded = HashSet::new();
    let mut count = 0usize;
    let mut expanded_bytes = 0u64;
    for entry in entries {
        let mut entry = entry.map_err(|err| {
            format!(
                "package.tarball_invalid: cannot read entry from `{}`: {err}",
                tarball.display()
            )
        })?;
        count = count.saturating_add(1);
        if count > MAX_PACKAGE_ARCHIVE_ENTRIES {
            return Err(format!(
                "package.archive_limit: archive contains more than {MAX_PACKAGE_ARCHIVE_ENTRIES} entries"
            ));
        }
        let path = std::str::from_utf8(&entry.path_bytes())
            .map_err(|_| {
                "package.tarball_invalid: archive member name is not valid UTF-8".to_string()
            })?
            .to_owned();
        let normalized = normalize_archive_member_path(&path)?;
        if !seen.insert(normalized.clone()) {
            return Err(format!(
                "package.tarball_invalid: duplicate archive member `{path}`"
            ));
        }
        let case_folded = normalized.to_ascii_lowercase();
        if !seen_case_folded.insert(case_folded) {
            return Err(format!(
                "package.tarball_invalid: archive member `{path}` collides by ASCII case"
            ));
        }
        let kind = entry.header().entry_type();
        if !(kind.is_file() || kind.is_dir()) {
            return Err(format!(
                "package.tarball_invalid: archive member `{path}` has unsupported entry kind `{:?}`",
                kind
            ));
        }
        expanded_bytes = expanded_bytes.checked_add(entry.size()).ok_or_else(|| {
            "package.archive_limit: expanded archive size overflowed u64".to_string()
        })?;
        if expanded_bytes > max_expanded_bytes {
            return Err(format!(
                "package.archive_limit: expanded archive exceeds {max_expanded_bytes} bytes"
            ));
        }
        let actual_bytes = std::io::copy(&mut entry, &mut std::io::sink()).map_err(|err| {
            format!("package.tarball_invalid: cannot read contents of member `{path}`: {err}")
        })?;
        if actual_bytes != entry.size() {
            return Err(format!(
                "package.tarball_invalid: member `{path}` declares {} bytes but contains {actual_bytes}",
                entry.size()
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
fn validate_archive_member_path(path: &str) -> Result<(), String> {
    normalize_archive_member_path(path).map(|_| ())
}

fn normalize_archive_member_path(path: &str) -> Result<String, String> {
    if path.is_empty() || path.len() > MAX_PACKAGE_ARCHIVE_PATH_BYTES {
        return Err(format!(
            "package.tarball_invalid: archive member path is empty or exceeds {MAX_PACKAGE_ARCHIVE_PATH_BYTES} bytes"
        ));
    }
    let path_without_trailing_slash = path.strip_suffix('/').unwrap_or(path);
    if path.starts_with('/')
        || path.starts_with('\\')
        || path.as_bytes().get(1) == Some(&b':')
        || path.contains('\0')
        || path.contains('\\')
        || path.split('/').any(|component| component == "..")
        || path_without_trailing_slash
            .split('/')
            .any(|component| component.is_empty())
    {
        return Err(format!(
            "package.tarball_invalid: archive member `{path}` escapes the extraction root"
        ));
    }
    if std::path::Path::new(path).components().any(|component| {
        matches!(
            component,
            std::path::Component::Prefix(_) | std::path::Component::RootDir
        )
    }) {
        return Err(format!(
            "package.tarball_invalid: archive member `{path}` uses an absolute path"
        ));
    }
    let components = path_without_trailing_slash
        .split('/')
        .filter(|component| !component.is_empty() && *component != ".")
        .collect::<Vec<_>>();
    if components.is_empty() {
        return Ok(".".to_string());
    }
    if components.len() > MAX_PACKAGE_ARCHIVE_DEPTH {
        return Err(format!(
            "package.archive_limit: archive member `{path}` exceeds depth {MAX_PACKAGE_ARCHIVE_DEPTH}"
        ));
    }
    Ok(components.join("/"))
}

#[cfg(test)]
fn validate_archive_expanded_size_with_limit(tarball: &Path, max_bytes: u64) -> Result<(), String> {
    validate_package_tarball_with_limit(tarball, max_bytes)
}

fn http_get_file(url: &str, dest: &Path, max_bytes: u64) -> Result<(), String> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "package.http_failed: cannot create `{}`: {err}",
                parent.display()
            )
        })?;
    }
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .build();
    let response = agent
        .get(url)
        .set(
            "User-Agent",
            concat!("ori-package/", env!("CARGO_PKG_VERSION")),
        )
        .call()
        .map_err(|err| format!("package.http_failed: GET `{url}` failed: {err}"))?;
    if let Some(length) = response
        .header("Content-Length")
        .and_then(|value| value.parse::<u64>().ok())
    {
        if length > max_bytes {
            return Err(format!(
                "package.download_limit: `{url}` declares {length} bytes, limit is {max_bytes}"
            ));
        }
    }
    let mut reader = response.into_reader();
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|err| {
            format!(
                "package.http_failed: exclusively create `{}`: {err}",
                dest.display()
            )
        })?;
    let result = (|| {
        let mut total = 0u64;
        let mut buffer = [0u8; 64 * 1024];
        loop {
            let read = reader
                .read(&mut buffer)
                .map_err(|err| format!("package.http_failed: read `{url}`: {err}"))?;
            if read == 0 {
                break;
            }
            total = total.saturating_add(read as u64);
            if total > max_bytes {
                return Err(format!(
                    "package.download_limit: `{url}` exceeds the {max_bytes}-byte limit"
                ));
            }
            output
                .write_all(&buffer[..read])
                .map_err(|err| format!("package.http_failed: write `{}`: {err}", dest.display()))?;
        }
        output
            .sync_all()
            .map_err(|err| format!("package.http_failed: sync `{}`: {err}", dest.display()))
    })();
    if result.is_err() {
        drop(output);
        let _ = fs::remove_file(dest);
    }
    result
}

fn parse_sha256_file(path: &Path) -> Result<String, String> {
    let source = fs::read_to_string(path).map_err(|err| {
        format!(
            "package.registry_digest_invalid: cannot read `{}`: {err}",
            path.display()
        )
    })?;
    let digest = source.split_whitespace().next().unwrap_or_default();
    if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(format!(
            "package.registry_digest_invalid: `{}` does not contain a SHA-256 digest",
            path.display()
        ));
    }
    Ok(digest.to_ascii_lowercase())
}

fn sha256_file(path: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let mut file = fs::File::open(path).map_err(|err| {
        format!(
            "package.digest_failed: cannot open `{}`: {err}",
            path.display()
        )
    })?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|err| {
            format!(
                "package.digest_failed: cannot read `{}`: {err}",
                path.display()
            )
        })?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn verify_file_sha256(path: &Path, expected: &str) -> Result<(), String> {
    let actual = sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(format!(
            "package.registry_digest_mismatch: `{}` hashed to {actual}, registry requires {expected}",
            path.display()
        ))
    }
}

fn http_put_file(url: &str, file: &Path, token: Option<&str>) -> Result<(), String> {
    let length = fs::metadata(file)
        .map_err(|err| {
            format!(
                "package.http_failed: cannot stat `{}`: {err}",
                file.display()
            )
        })?
        .len();
    let body = fs::File::open(file).map_err(|err| {
        format!(
            "package.http_failed: cannot open `{}`: {err}",
            file.display()
        )
    })?;
    let agent = ureq::AgentBuilder::new()
        .redirects(0)
        .timeout_connect(Duration::from_secs(10))
        .timeout_read(Duration::from_secs(30))
        .timeout_write(Duration::from_secs(30))
        .build();
    let mut request = agent
        .put(url)
        .set(
            "User-Agent",
            concat!("ori-package/", env!("CARGO_PKG_VERSION")),
        )
        .set("If-None-Match", "*")
        .set("Content-Length", &length.to_string());
    if let Some(token) = token {
        // Keep credentials in the request object rather than command-line
        // arguments visible to other local processes.
        request = request.set("Authorization", &format!("Bearer {token}"));
    }
    match request.send(body) {
        Ok(_) => Ok(()),
        Err(ureq::Error::Status(409 | 412, _)) => Err(format!(
            "package.publish_exists: `{url}` already exists; package versions are immutable"
        )),
        Err(err) => Err(format!("package.http_failed: PUT `{url}` failed: {err}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_indexes_fail_closed_and_replace_atomically() {
        let root = std::env::temp_dir().join(format!(
            "ori_registry_index_{}_{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let package_dir = root.join("packages/demo");
        fs::create_dir_all(&package_dir).expect("create registry fixture");
        let versions = package_dir.join("versions.json");
        fs::write(&versions, "not-json").expect("write malformed versions index");
        let error = update_versions_index(&root, "demo", "1.0.0")
            .expect_err("malformed index must not be silently replaced");
        assert!(error.contains("registry_index_invalid"), "{error}");
        assert_eq!(
            fs::read_to_string(&versions).expect("read index"),
            "not-json"
        );

        fs::write(&versions, "{\"versions\": [\"1.0.0\"]}").expect("write valid index");
        update_versions_index(&root, "demo", "1.1.0").expect("atomic index update");
        let text = fs::read_to_string(&versions).expect("read updated index");
        assert!(text.contains("1.0.0") && text.contains("1.1.0"));
        let _ = fs::remove_dir_all(root);
    }

    fn write_hostile_archive(path: &Path, entries: &[(&str, tar::EntryType, &[u8], Option<&str>)]) {
        let file = fs::File::create(path).expect("create hostile archive");
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        for (entry_path, entry_type, body, link_name) in entries {
            let mut header = tar::Header::new_gnu();
            header.set_mode(0o644);
            header.set_size(body.len() as u64);
            header.set_entry_type(*entry_type);
            if let Some(link_name) = link_name {
                header
                    .set_link_name(link_name)
                    .expect("set hostile link name");
            }
            header.set_cksum();
            builder
                .append_data(&mut header, entry_path, *body)
                .expect("append hostile entry");
        }
        let encoder = builder.into_inner().expect("finish hostile tar");
        encoder.finish().expect("finish hostile gzip");
    }

    #[test]
    fn lockfile_round_trip_preserves_path_dependency() {
        let root = std::env::temp_dir().join(format!(
            "ori_lock_round_trip_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after epoch")
                .as_nanos()
        ));
        let dependency = root.join("dep");
        fs::create_dir_all(&dependency).expect("create test package");
        fs::write(
            dependency.join("ori.pkg.toml"),
            "[package]\nname = \"demo.dep\"\nversion = \"1.0.0\"\nentry = \"main.orl\"\nori_version = \"0.3.8\"\n",
        )
        .expect("write dependency manifest");
        fs::write(dependency.join("main.orl"), "module demo.dep\n")
            .expect("write dependency source");
        fs::create_dir_all(&root).expect("create root package");
        fs::write(
            root.join("ori.pkg.toml"),
            "[package]\nname = \"demo.app\"\nversion = \"1.0.0\"\nentry = \"main.orl\"\nori_version = \"0.3.8\"\n\n[dependencies]\ndemo.dep = { path = \"dep\" }\n",
        )
        .expect("write root manifest");
        fs::write(root.join("main.orl"), "module demo.app\n").expect("write root source");

        let output = run_lock_package(LockPackageOptions {
            path: root.clone(),
            locked: false,
            cache_root: None,
            offline: false,
        })
        .expect("write lockfile");
        assert!(output.changed);
        let lock = load_package_lock(&output.path).expect("read lockfile");
        assert_eq!(lock.root_name, "demo.app");
        assert_eq!(lock.dependencies.len(), 1);
        assert_eq!(lock.dependencies[0].source, LockedDependencySource::Path);
        assert_eq!(lock.dependencies[0].path.as_deref(), Some("dep"));

        let second = run_lock_package(LockPackageOptions {
            path: root.clone(),
            locked: true,
            cache_root: None,
            offline: false,
        })
        .expect("validate unchanged lockfile");
        assert!(!second.changed);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn unlocked_lock_command_migrates_v1_but_locked_mode_refuses_it() {
        let root = std::env::temp_dir().join(format!(
            "ori_lock_migrate_{}_{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create package root");
        fs::write(root.join("main.orl"), "module demo.migrate\n").expect("write entry");
        fs::write(
            root.join("ori.pkg.toml"),
            "[package]\nname = \"demo.migrate\"\nversion = \"1.0.0\"\nentry = \"main.orl\"\nori_version = \"0.3.8\"\n",
        )
        .expect("write manifest");
        let lock_path = root.join("ori.lock");
        fs::write(
            &lock_path,
            "# legacy lock\nformat = 1\nroot = \"demo.migrate\"\nroot_version = \"1.0.0\"\n",
        )
        .expect("write v1 lock");

        let locked_error = run_lock_package(LockPackageOptions {
            path: root.clone(),
            locked: true,
            cache_root: None,
            offline: true,
        })
        .expect_err("locked mode cannot accept a digest-free v1 lock");
        assert!(
            locked_error.contains("package.lock_version"),
            "{locked_error}"
        );

        let migrated = run_lock_package(LockPackageOptions {
            path: root.clone(),
            locked: false,
            cache_root: None,
            offline: false,
        })
        .expect("explicit unlocked lock command migrates v1");
        assert!(migrated.changed);
        load_package_lock(&lock_path).expect("migrated v2 lock is readable");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn package_manifest_declares_cfg_features_and_defaults() {
        let root = std::env::temp_dir().join(format!(
            "ori_package_features_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create package root");
        fs::write(root.join("main.orl"), "module demo.features\n").expect("write entry");
        fs::write(
            root.join("ori.pkg.toml"),
            "[package]\nname = \"demo.features\"\nversion = \"1.0.0\"\nentry = \"main.orl\"\nori_version = \"0.3.8\"\n\n[features]\ndefault = [\"tls\"]\ntls = []\ntelemetry = []\n",
        )
        .expect("write manifest");

        let manifest = load_package_manifest(&root).expect("parse package features");
        assert_eq!(
            manifest.default_features,
            BTreeSet::from(["tls".to_string()])
        );
        assert_eq!(
            manifest.declared_features,
            BTreeSet::from(["telemetry".to_string(), "tls".to_string()])
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_member_validation_rejects_escape_and_links() {
        assert!(validate_archive_member_path("src/main.orl").is_ok());
        assert!(validate_archive_member_path("../outside").is_err());
        assert!(validate_archive_member_path("/outside").is_err());
        assert!(validate_archive_member_path("C:/outside").is_err());
        assert!(validate_archive_member_path("src//main.orl").is_err());
        assert!(validate_archive_member_path("src/../main.orl").is_err());
    }

    #[test]
    fn valid_package_tarball_passes_preflight_before_extraction() {
        let root = std::env::temp_dir().join(format!(
            "ori_tarball_preflight_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("system clock must be after epoch")
                .as_nanos()
        ));
        let source = root.join("source");
        let dest = root.join("dest");
        let archive = root.join("package.tar.gz");
        fs::create_dir_all(&source).expect("create source");
        fs::write(source.join("ori.pkg.toml"), "[package]\n").expect("write manifest");
        fs::write(source.join("main.orl"), "module demo.pkg\n").expect("write source");
        create_package_tarball(&source, &archive).expect("create archive");
        fs::create_dir_all(&dest).expect("create destination");
        extract_package_tarball(&archive, &dest).expect("validated extraction");
        assert!(dest.join("ori.pkg.toml").is_file());
        assert!(dest.join("main.orl").is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lockfile_digest_rejects_changed_path_dependency() {
        let root = std::env::temp_dir().join(format!(
            "ori_lock_digest_{}_{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let dependency = root.join("dep");
        fs::create_dir_all(&dependency).expect("create dependency");
        fs::write(
            dependency.join("ori.pkg.toml"),
            "[package]\nname = \"demo.dep\"\nversion = \"1.0.0\"\nentry = \"main.orl\"\nori_version = \"0.3.8\"\n",
        )
        .expect("write dependency manifest");
        fs::write(dependency.join("main.orl"), "module demo.dep\n")
            .expect("write dependency source");
        fs::write(
            root.join("ori.pkg.toml"),
            "[package]\nname = \"demo.app\"\nversion = \"1.0.0\"\nentry = \"main.orl\"\nori_version = \"0.3.8\"\n\n[dependencies]\ndemo.dep = { path = \"dep\" }\n",
        )
        .expect("write root manifest");
        fs::write(root.join("main.orl"), "module demo.app\n").expect("write root source");
        run_lock_package(LockPackageOptions {
            path: root.clone(),
            locked: false,
            cache_root: None,
            offline: false,
        })
        .expect("write lock");
        fs::write(dependency.join("main.orl"), "module demo.dep\n// changed\n")
            .expect("tamper dependency");
        let error = run_lock_package(LockPackageOptions {
            path: root.clone(),
            locked: true,
            cache_root: None,
            offline: true,
        })
        .expect_err("changed path dependency must fail locked validation");
        assert!(error.contains("package.lock_digest_mismatch"), "{error}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn lockfile_restores_transitive_paths_relative_to_project_root() {
        let root = std::env::temp_dir().join(format!(
            "ori_lock_transitive_path_{}_{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let first = root.join("deps/first");
        let second = root.join("shared/second");
        fs::create_dir_all(&first).expect("create first dependency");
        fs::create_dir_all(&second).expect("create second dependency");
        fs::write(first.join("main.orl"), "module demo.first\n").expect("write first entry");
        fs::write(second.join("main.orl"), "module demo.second\n").expect("write second entry");
        fs::write(
            first.join("ori.pkg.toml"),
            "[package]\nname = \"demo.first\"\nversion = \"1.0.0\"\nentry = \"main.orl\"\nori_version = \"0.3.8\"\n\n[dependencies]\ndemo.second = { path = \"../../shared/second\" }\n",
        )
        .expect("write first manifest");
        fs::write(
            second.join("ori.pkg.toml"),
            "[package]\nname = \"demo.second\"\nversion = \"1.0.0\"\nentry = \"main.orl\"\nori_version = \"0.3.8\"\n",
        )
        .expect("write second manifest");
        fs::write(root.join("main.orl"), "module demo.app\n").expect("write root entry");
        fs::write(
            root.join("ori.pkg.toml"),
            "[package]\nname = \"demo.app\"\nversion = \"1.0.0\"\nentry = \"main.orl\"\nori_version = \"0.3.8\"\n\n[dependencies]\ndemo.first = { path = \"deps/first\" }\n",
        )
        .expect("write root manifest");

        run_lock_package(LockPackageOptions {
            path: root.clone(),
            locked: false,
            cache_root: None,
            offline: false,
        })
        .expect("lock transitive paths");
        let lock = load_package_lock(&root.join("ori.lock")).expect("read lock");
        let transitive = lock
            .dependencies
            .iter()
            .find(|dependency| dependency.name == "demo.second")
            .expect("transitive dependency is locked");
        assert_eq!(transitive.url.as_deref(), Some("path:shared/second"));
        run_lock_package(LockPackageOptions {
            path: root.clone(),
            locked: true,
            cache_root: None,
            offline: true,
        })
        .expect("restore project-relative transitive path");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cache_digest_and_source_identity_reject_tampering_and_collisions() {
        let root = std::env::temp_dir().join(format!(
            "ori_cache_identity_{}_{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let first = root.join("first");
        let second = root.join("second");
        let target = root.join("cache/demo.same/1.0.0");
        for (source, body) in [(&first, "first"), (&second, "second")] {
            fs::create_dir_all(source).expect("create source");
            fs::write(
                source.join("ori.pkg.toml"),
                "[package]\nname = \"demo.same\"\nversion = \"1.0.0\"\nentry = \"main.orl\"\nori_version = \"0.3.8\"\n",
            )
            .expect("write manifest");
            fs::write(source.join("main.orl"), body).expect("write source");
        }
        publish_cache_entry(&first, &target, "registry:https://first.invalid", None)
            .expect("publish first cache entry");
        let collision =
            publish_cache_entry(&second, &target, "registry:https://second.invalid", None)
                .expect_err("different sources cannot share name/version cache identity");
        assert!(
            collision.contains("package.cache_source_mismatch"),
            "{collision}"
        );

        fs::write(target.join("main.orl"), "tampered").expect("tamper cache");
        let tampered = validate_cache_entry(
            &target,
            "demo.same",
            "1.0.0",
            Some("registry:https://first.invalid"),
            None,
        )
        .expect_err("changed cache byte must fail validation");
        assert!(
            tampered.contains("package.cache_digest_mismatch"),
            "{tampered}"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_preflight_rejects_case_collision_truncation_and_expansion_limit() {
        let root = std::env::temp_dir().join(format!(
            "ori_archive_hostile_{}_{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        let collision = root.join("collision.tar.gz");
        fs::create_dir_all(&root).expect("create hostile root");
        write_hostile_archive(
            &collision,
            &[
                ("Case.orl", tar::EntryType::Regular, b"A", None),
                ("case.orl", tar::EntryType::Regular, b"B", None),
            ],
        );
        let error = validate_package_tarball(&collision).expect_err("case collision must fail");
        assert!(error.contains("collides by ASCII case"), "{error}");

        let source = root.join("source");
        fs::create_dir_all(&source).expect("create source");
        fs::write(source.join("Case.orl"), "A").expect("write valid source");
        let valid = root.join("valid.tar.gz");
        create_package_tarball(&source, &valid).expect("create valid archive");
        let expansion = validate_archive_expanded_size_with_limit(&valid, 0)
            .expect_err("expanded size limit must fail before extraction");
        assert!(expansion.contains("package.archive_limit"), "{expansion}");

        let truncated = root.join("truncated.tar.gz");
        let bytes = fs::read(&valid).expect("read archive");
        fs::write(&truncated, &bytes[..bytes.len() / 2]).expect("write truncated archive");
        let error = validate_package_tarball(&truncated).expect_err("truncation must fail");
        assert!(error.contains("package.tarball_invalid"), "{error}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn archive_preflight_rejects_symlink_and_hardlink_entries() {
        let root = std::env::temp_dir().join(format!(
            "ori_archive_links_{}_{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create hostile root");
        let symlink_archive = root.join("symlink.tar.gz");
        write_hostile_archive(
            &symlink_archive,
            &[("link.orl", tar::EntryType::Symlink, b"", Some("target.orl"))],
        );
        let error = validate_package_tarball(&symlink_archive).expect_err("symlink must fail");
        assert!(error.contains("unsupported entry kind"), "{error}");

        let hardlink_archive = root.join("hardlink.tar.gz");
        write_hostile_archive(
            &hardlink_archive,
            &[("hard.orl", tar::EntryType::Link, b"", Some("target.orl"))],
        );
        let error = validate_package_tarball(&hardlink_archive).expect_err("hardlink must fail");
        assert!(error.contains("unsupported entry kind"), "{error}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn package_manifest_parses_declarative_native_dependencies() {
        let manifest_src = r#"
[package]
name = "engine_demo"
version = "0.2.0"
entry = "main.orl"
ori_version = "0.3.8"

[native.dependencies.raylib]
pkg_config = "raylib"
static = true
version = ">= 5.0"

[native.dependencies]
gl = { pkg_config = "gl" }
cocoa = { framework = "Cocoa" }
box2d = "box2d"

[native.linux]
libraries = ["GL", "X11", "m", "dl"]
library_dirs = ["/usr/local/lib"]
link_flags = ["-Wl,-rpath,/opt/engine/lib"]

[native.windows]
libraries = ["user32", "opengl32"]
library_dirs = ["C:\\libs"]
link_flags = ["/NODEFAULTLIB:libcmt"]

[native.macos]
frameworks = ["OpenGL", "Cocoa", "IOKit"]
libraries = ["m"]
library_dirs = ["/opt/homebrew/lib"]

[native]
libraries = ["common_c"]
link_flags = ["-Wl,-z,now"]
"#;
        let root = std::env::temp_dir().join(format!(
            "ori_pkg_native_{}_{}",
            std::process::id(),
            TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&root).expect("create test root");
        fs::write(root.join("main.orl"), "module app.main\nmain()\nend\n").expect("write entry");
        let manifest_path = root.join("ori.pkg.toml");
        fs::write(&manifest_path, manifest_src).expect("write manifest");

        let manifest = load_package_manifest(&manifest_path).expect("load manifest must succeed");
        assert_eq!(manifest.name, "engine_demo");
        assert_eq!(manifest.version, "0.2.0");

        // Assert native dependencies
        let raylib = manifest
            .native_config
            .dependencies
            .iter()
            .find(|d| d.name == "raylib")
            .expect("raylib dep");
        assert_eq!(raylib.pkg_config.as_deref(), Some("raylib"));
        assert!(raylib.is_static);
        assert_eq!(raylib.version.as_deref(), Some(">= 5.0"));

        let gl = manifest
            .native_config
            .dependencies
            .iter()
            .find(|d| d.name == "gl")
            .expect("gl dep");
        assert_eq!(gl.pkg_config.as_deref(), Some("gl"));
        assert!(!gl.is_static);

        let cocoa = manifest
            .native_config
            .dependencies
            .iter()
            .find(|d| d.name == "cocoa")
            .expect("cocoa dep");
        assert_eq!(cocoa.framework.as_deref(), Some("Cocoa"));

        let box2d = manifest
            .native_config
            .dependencies
            .iter()
            .find(|d| d.name == "box2d")
            .expect("box2d dep");
        assert_eq!(box2d.pkg_config.as_deref(), Some("box2d"));

        // Assert platform link configurations
        assert_eq!(
            manifest.native_config.platforms.linux.libraries,
            vec!["GL", "X11", "m", "dl"]
        );
        assert_eq!(
            manifest.native_config.platforms.linux.library_dirs,
            vec![PathBuf::from("/usr/local/lib")]
        );
        assert_eq!(
            manifest.native_config.platforms.linux.link_flags,
            vec!["-Wl,-rpath,/opt/engine/lib"]
        );

        assert_eq!(
            manifest.native_config.platforms.windows.libraries,
            vec!["user32", "opengl32"]
        );
        assert_eq!(
            manifest.native_config.platforms.windows.library_dirs,
            vec![PathBuf::from("C:\\libs")]
        );
        assert_eq!(
            manifest.native_config.platforms.windows.link_flags,
            vec!["/NODEFAULTLIB:libcmt"]
        );

        assert_eq!(
            manifest.native_config.platforms.macos.frameworks,
            vec!["OpenGL", "Cocoa", "IOKit"]
        );
        assert_eq!(manifest.native_config.platforms.macos.libraries, vec!["m"]);
        assert_eq!(
            manifest.native_config.platforms.macos.library_dirs,
            vec![PathBuf::from("/opt/homebrew/lib")]
        );

        assert_eq!(
            manifest.native_config.platforms.all.libraries,
            vec!["common_c"]
        );
        assert_eq!(
            manifest.native_config.platforms.all.link_flags,
            vec!["-Wl,-z,now"]
        );

        let _ = fs::remove_dir_all(root);
    }
}
