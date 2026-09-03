//! Declarative native dependencies and platform link flag resolution (`PKG-NATIVE-1`).
//!
//! Handles resolution of `[native.dependencies]` via `pkg-config` and explicit
//! per-platform library/framework link configurations (`[native.linux]`,
//! `[native.windows]`, `[native.macos]`, and `[native]`).

use crate::package::{NativeConfig, NativeDependency, PlatformLinkConfig};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Consolidated native link arguments resolved for a specific compilation target.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResolvedNativeLink {
    pub libraries: Vec<String>,
    pub frameworks: Vec<String>,
    pub library_dirs: Vec<PathBuf>,
    pub link_flags: Vec<String>,
}

impl ResolvedNativeLink {
    /// Merge another resolved link specification into this one without duplicate entries.
    pub fn merge(&mut self, other: ResolvedNativeLink) {
        for dir in other.library_dirs {
            if !self.library_dirs.contains(&dir) {
                self.library_dirs.push(dir);
            }
        }
        for lib in other.libraries {
            if !self.libraries.contains(&lib) {
                self.libraries.push(lib);
            }
        }
        for fw in other.frameworks {
            if !self.frameworks.contains(&fw) {
                self.frameworks.push(fw);
            }
        }
        for flag in other.link_flags {
            if !self.link_flags.contains(&flag) {
                self.link_flags.push(flag);
            }
        }
    }

    /// Convert the resolved dependencies into linker command line arguments
    /// appropriate for the target triple.
    pub fn to_link_args(&self, target_triple: &str) -> Vec<String> {
        let is_msvc = target_triple.contains("windows-msvc");
        let is_macos = target_triple.contains("darwin") || target_triple.contains("apple");
        let mut args = Vec::new();

        for dir in &self.library_dirs {
            if is_msvc {
                args.push(format!("/LIBPATH:{}", dir.display()));
            } else {
                args.push(format!("-L{}", dir.display()));
            }
        }

        for lib in &self.libraries {
            if is_msvc {
                if lib.ends_with(".lib") || lib.ends_with(".a") {
                    args.push(lib.clone());
                } else {
                    args.push(format!("{lib}.lib"));
                }
            } else if lib.starts_with("-l") {
                args.push(lib.clone());
            } else {
                args.push(format!("-l{lib}"));
            }
        }

        if is_macos {
            for fw in &self.frameworks {
                args.push("-framework".to_string());
                args.push(fw.clone());
            }
        }

        for flag in &self.link_flags {
            args.push(flag.clone());
        }

        args
    }
}

/// Resolve all native dependencies and platform configurations declared in `config`
/// for `target_triple` using `package_root` to resolve relative paths.
pub fn resolve_native_config(
    config: &NativeConfig,
    target_triple: &str,
    package_root: &Path,
) -> Result<ResolvedNativeLink, String> {
    let mut resolved = ResolvedNativeLink::default();

    // 1. Resolve declared native dependencies (e.g. via pkg-config)
    for dep in &config.dependencies {
        resolve_single_native_dependency(dep, target_triple, &mut resolved)?;
    }

    // 2. Select matching platform configuration
    let platform_config = select_platform_config(config, target_triple);
    apply_platform_config(&mut resolved, platform_config, package_root);

    // 3. Always apply platform-independent common configuration (`[native]`)
    apply_platform_config(&mut resolved, &config.platforms.all, package_root);

    Ok(resolved)
}

fn select_platform_config<'a>(
    config: &'a NativeConfig,
    target_triple: &str,
) -> &'a PlatformLinkConfig {
    if target_triple.contains("linux") {
        &config.platforms.linux
    } else if target_triple.contains("windows") {
        &config.platforms.windows
    } else if target_triple.contains("darwin") || target_triple.contains("apple") {
        &config.platforms.macos
    } else {
        &config.platforms.all
    }
}

fn apply_platform_config(
    resolved: &mut ResolvedNativeLink,
    platform: &PlatformLinkConfig,
    package_root: &Path,
) {
    for dir in &platform.library_dirs {
        let abs_dir = if dir.is_absolute() {
            dir.clone()
        } else {
            package_root.join(dir)
        };
        if !resolved.library_dirs.contains(&abs_dir) {
            resolved.library_dirs.push(abs_dir);
        }
    }

    for lib in &platform.libraries {
        if !resolved.libraries.contains(lib) {
            resolved.libraries.push(lib.clone());
        }
    }

    for fw in &platform.frameworks {
        if !resolved.frameworks.contains(fw) {
            resolved.frameworks.push(fw.clone());
        }
    }

    for flag in &platform.link_flags {
        if !resolved.link_flags.contains(flag) {
            resolved.link_flags.push(flag.clone());
        }
    }
}

fn resolve_single_native_dependency(
    dep: &NativeDependency,
    target_triple: &str,
    resolved: &mut ResolvedNativeLink,
) -> Result<(), String> {
    if let Some(pkg_name) = &dep.pkg_config {
        resolve_via_pkg_config(dep, pkg_name, resolved)?;
        return Ok(());
    }

    if let Some(fw) = &dep.framework {
        let is_macos = target_triple.contains("darwin") || target_triple.contains("apple");
        if is_macos && !resolved.frameworks.contains(fw) {
            resolved.frameworks.push(fw.clone());
        }
        return Ok(());
    }

    // Default: treated as direct library name
    if !resolved.libraries.contains(&dep.name) {
        resolved.libraries.push(dep.name.clone());
    }

    Ok(())
}

fn resolve_via_pkg_config(
    dep: &NativeDependency,
    pkg_name: &str,
    resolved: &mut ResolvedNativeLink,
) -> Result<(), String> {
    let pkg_config_cmd = std::env::var("PKG_CONFIG").unwrap_or_else(|_| "pkg-config".to_string());

    // Check version requirement if specified
    if let Some(ver_req) = &dep.version {
        let trimmed_req = ver_req.trim();
        let (cmp_op, target_version) = if let Some(rest) = trimmed_req.strip_prefix(">=") {
            ("--atleast-version", rest.trim())
        } else if let Some(rest) = trimmed_req.strip_prefix("<=") {
            ("--max-version", rest.trim())
        } else if let Some(rest) = trimmed_req.strip_prefix("=") {
            ("--exact-version", rest.trim())
        } else {
            ("--atleast-version", trimmed_req)
        };

        let status = Command::new(&pkg_config_cmd)
            .arg(format!("{cmp_op}={target_version}"))
            .arg(pkg_name)
            .status()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    format!(
                        "package.native_pkg_config_missing: `pkg-config` tool is not available in PATH, required for native dependency `{}`",
                        dep.name
                    )
                } else {
                    format!(
                        "package.native_pkg_config_failed: failed to execute `pkg-config`: {e}"
                    )
                }
            })?;

        if !status.success() {
            // Retrieve installed version for better diagnostic
            let installed_version = Command::new(&pkg_config_cmd)
                .arg("--modversion")
                .arg(pkg_name)
                .output()
                .ok()
                .and_then(|out| {
                    if out.status.success() {
                        String::from_utf8(out.stdout).ok()
                    } else {
                        None
                    }
                })
                .map(|v| v.trim().to_string())
                .unwrap_or_else(|| "not found".to_string());

            return Err(format!(
                "package.native_dependency_version_mismatch: native dependency `{}` requirement `{ver_req}` was not satisfied (installed: `{installed_version}`)",
                dep.name
            ));
        }
    }

    // Query linker flags from pkg-config
    let mut cmd = Command::new(&pkg_config_cmd);
    cmd.arg("--libs");
    if dep.is_static {
        cmd.arg("--static");
    }
    cmd.arg(pkg_name);

    let output = cmd.output().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            format!(
                "package.native_pkg_config_missing: `pkg-config` tool is not available in PATH, required for native dependency `{}`",
                dep.name
            )
        } else {
            format!(
                "package.native_pkg_config_failed: failed to execute `pkg-config`: {e}"
            )
        }
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "package.native_dependency_missing: `pkg-config` failed to resolve package `{pkg_name}` for dependency `{}`: {}",
            dep.name,
            stderr.trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_pkg_config_libs_output(&stdout, resolved);

    Ok(())
}

/// Parse the stdout from `pkg-config --libs` into structured link items.
pub fn parse_pkg_config_libs_output(output: &str, resolved: &mut ResolvedNativeLink) {
    let tokens = tokenize_command_args(output);
    let mut iter = tokens.into_iter().peekable();

    while let Some(token) = iter.next() {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }

        if let Some(dir) = token.strip_prefix("-L") {
            let path = PathBuf::from(dir);
            if !resolved.library_dirs.contains(&path) {
                resolved.library_dirs.push(path);
            }
        } else if let Some(lib) = token.strip_prefix("-l") {
            let lib_name = lib.to_string();
            if !resolved.libraries.contains(&lib_name) {
                resolved.libraries.push(lib_name);
            }
        } else if token == "-framework" {
            if let Some(fw) = iter.next() {
                let fw_name = fw.trim().to_string();
                if !resolved.frameworks.contains(&fw_name) {
                    resolved.frameworks.push(fw_name);
                }
            }
        } else {
            // General link flags (e.g. -pthread, -Wl,-rpath,...)
            let flag = token.to_string();
            if !resolved.link_flags.contains(&flag) {
                resolved.link_flags.push(flag);
            }
        }
    }
}

/// Split shell command arguments respecting single and double quotes.
fn tokenize_command_args(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single_quote = false;
    let mut in_double_quote = false;
    let mut escaped = false;

    for ch in input.chars() {
        if escaped {
            current.push(ch);
            escaped = false;
            continue;
        }

        if ch == '\\' && !in_single_quote {
            escaped = true;
            continue;
        }

        if ch == '\'' && !in_double_quote {
            in_single_quote = !in_single_quote;
            continue;
        }

        if ch == '"' && !in_single_quote {
            in_double_quote = !in_double_quote;
            continue;
        }

        if ch.is_whitespace() && !in_single_quote && !in_double_quote {
            if !current.is_empty() {
                tokens.push(std::mem::take(&mut current));
            }
            continue;
        }

        current.push(ch);
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_pkg_config_libs_output() {
        let raw = "-L/usr/local/lib -lraylib -lGL -lm -lpthread -ldl -lrt -lX11";
        let mut resolved = ResolvedNativeLink::default();
        parse_pkg_config_libs_output(raw, &mut resolved);

        assert_eq!(resolved.library_dirs, vec![PathBuf::from("/usr/local/lib")]);
        assert_eq!(
            resolved.libraries,
            vec!["raylib", "GL", "m", "pthread", "dl", "rt", "X11"]
        );
    }

    #[test]
    fn test_parse_pkg_config_with_frameworks() {
        let raw = "-L/opt/homebrew/lib -framework OpenGL -framework Cocoa -lraylib";
        let mut resolved = ResolvedNativeLink::default();
        parse_pkg_config_libs_output(raw, &mut resolved);

        assert_eq!(
            resolved.library_dirs,
            vec![PathBuf::from("/opt/homebrew/lib")]
        );
        assert_eq!(resolved.frameworks, vec!["OpenGL", "Cocoa"]);
        assert_eq!(resolved.libraries, vec!["raylib"]);
    }

    #[test]
    fn test_to_link_args_linux() {
        let resolved = ResolvedNativeLink {
            libraries: vec!["GL".to_string(), "m".to_string()],
            frameworks: vec![],
            library_dirs: vec![PathBuf::from("/usr/lib/custom")],
            link_flags: vec!["-Wl,-rpath,/usr/lib/custom".to_string()],
        };

        let args = resolved.to_link_args("x86_64-unknown-linux-gnu");
        assert_eq!(
            args,
            vec![
                "-L/usr/lib/custom",
                "-lGL",
                "-lm",
                "-Wl,-rpath,/usr/lib/custom"
            ]
        );
    }

    #[test]
    fn test_to_link_args_windows_msvc() {
        let resolved = ResolvedNativeLink {
            libraries: vec!["user32".to_string(), "opengl32.lib".to_string()],
            frameworks: vec![],
            library_dirs: vec![PathBuf::from("C:\\libs")],
            link_flags: vec!["/NODEFAULTLIB:libcmt".to_string()],
        };

        let args = resolved.to_link_args("x86_64-pc-windows-msvc");
        assert_eq!(
            args,
            vec![
                "/LIBPATH:C:\\libs",
                "user32.lib",
                "opengl32.lib",
                "/NODEFAULTLIB:libcmt"
            ]
        );
    }

    #[test]
    fn test_resolve_platform_selection() {
        let mut config = NativeConfig::default();
        config.platforms.linux.libraries = vec!["GL".to_string(), "X11".to_string()];
        config.platforms.windows.libraries = vec!["opengl32".to_string()];
        config.platforms.all.libraries = vec!["common_math".to_string()];

        let root = Path::new("/workspace");
        let linux_link = resolve_native_config(&config, "x86_64-unknown-linux-gnu", root).unwrap();
        assert_eq!(linux_link.libraries, vec!["GL", "X11", "common_math"]);

        let win_link = resolve_native_config(&config, "x86_64-pc-windows-msvc", root).unwrap();
        assert_eq!(win_link.libraries, vec!["opengl32", "common_math"]);
    }
}
