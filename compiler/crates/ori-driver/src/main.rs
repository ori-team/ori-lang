use clap::{Parser, Subcommand};
use ori_driver::{emit, explain, package, pipeline, update};
use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Parser)]
#[command(
    name    = "ori",
    version = env!("CARGO_PKG_VERSION"),
    about   = "The Ori language compiler",
    long_about = None,
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Disable ANSI colour output.
    #[arg(long, global = true)]
    no_color: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a new Ori project.
    New {
        /// Directory to create.
        path: PathBuf,
        /// Project name written to `ori.pkg.toml` (default: directory name).
        #[arg(long)]
        name: Option<String>,
        /// Create a library project instead of an app project.
        #[arg(long)]
        lib: bool,
    },
    /// Initialize a new Ori project in an existing directory.
    Init {
        /// Directory to initialize (default: current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Project name written to `ori.pkg.toml` (default: directory name).
        #[arg(long)]
        name: Option<String>,
        /// Create a library project instead of an app project.
        #[arg(long)]
        lib: bool,
    },
    /// Type-check an Ori source file and report diagnostics.
    Check {
        /// Path to the `.orl` source file.
        file: PathBuf,
    },
    /// Extract documentation comments as Markdown or static HTML.
    Doc {
        #[command(subcommand)]
        action: DocAction,
    },
    /// Install an Ori package into the local package cache.
    ///
    /// Sources: `--path` (local), GitHub URL, or registry name (`name` / `name@version`
    /// when `ORI_REGISTRY` is set).
    Install {
        /// Package name (`demo.math`), `name@version`, or GitHub URL.
        name: String,
        /// Local package directory or `ori.pkg.toml` path.
        #[arg(long)]
        path: Option<PathBuf>,
        /// Override the package cache root (default: ORI_PACKAGE_CACHE or ~/.ori/packages).
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// Fetch git/path dependencies declared by a project or package into the local cache.
    ///
    /// Reads `ori.proj` or `ori.pkg.toml` under `path` (default `.`) and materializes
    /// `{ git = "..." }` dependencies (and path deps into the versioned cache layout).
    /// Check/build also auto-fetch git deps when the cache is configured.
    Get {
        /// Project/package root, `ori.proj`, or `ori.pkg.toml` (default: current directory).
        #[arg(default_value = ".")]
        path: PathBuf,
        /// Override the package cache root (default: ORI_PACKAGE_CACHE or ~/.ori/packages).
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// Publish an Ori package to the configured registry (`ORI_REGISTRY`).
    ///
    /// File registry (directory path): copies the package tree under
    /// `{registry}/packages/{name}/{version}/` and writes `versions.json`.
    /// HTTP(S) registry: uploads `{base}/packages/{name}/{version}.tar.gz` via PUT
    /// (optional `ORI_REGISTRY_TOKEN` / `--token` Bearer auth).
    Publish {
        /// Path to the package root or `ori.pkg.toml`.
        path: PathBuf,
        /// Override `ORI_REGISTRY` (directory path or http(s) base URL).
        #[arg(long)]
        registry: Option<String>,
        /// Bearer token for HTTP publish (`ORI_REGISTRY_TOKEN`).
        #[arg(long)]
        token: Option<String>,
        /// Replace an existing `name@version` in the registry.
        #[arg(long)]
        force: bool,
    },
    /// Run functions marked with `@test` through the native runtime.
    Test {
        /// Path to the `.orl` source file or project manifest.
        file: PathBuf,
        /// Run only tests whose fully-qualified or short name contains this text.
        #[arg(long)]
        filter: Option<String>,
    },
    /// Format an Ori source file and print the result.
    Fmt {
        /// Path to the `.orl` source file.
        file: PathBuf,
    },
    /// Print the raw token stream (debug).
    Lex {
        /// Path to the `.orl` source file.
        file: PathBuf,
    },
    /// Print the AST (debug).
    Parse {
        /// Path to the `.orl` source file.
        file: PathBuf,
    },
    /// Compile to a native binary via Cranelift and the packaged native runtime.
    Compile {
        /// Path to the `.orl` source file.
        file: PathBuf,
        /// Output path (default: source name; with `--lib`: `lib<name>.so` / `.dll` / `.dylib`).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Emit a shared library with C ABI exports (`@c_export`) for embed hosts.
        #[arg(long)]
        lib: bool,
        /// Print full native linker stdout/stderr when link fails.
        #[arg(long)]
        native_raw: bool,
    },
    /// Compile and run an Ori source file through the native runtime.
    Run {
        /// Path to the `.orl` source file.
        file: PathBuf,
        /// Print full native linker stdout/stderr when link fails.
        #[arg(long)]
        native_raw: bool,
    },
    /// Start a small interactive REPL backed by the native JIT.
    Repl,
    /// Build an Ori file or project through the native backend.
    Build {
        /// Path to the `.orl` source file, `ori.proj`, or project root.
        path: PathBuf,
        /// Output executable path (default: same name as source, no extension).
        #[arg(short, long)]
        out: Option<PathBuf>,
        /// Print full native linker stdout/stderr when link fails.
        #[arg(long)]
        native_raw: bool,
    },
    /// Emit secondary debug artifacts.
    Emit {
        #[command(subcommand)]
        action: EmitAction,
    },
    /// Report environment, stdlib, and native runtime health.
    Doctor,
    /// Update the Ori toolchain to the latest published release.
    Update {
        /// Check for a newer release without installing it.
        #[arg(long)]
        check: bool,
    },
    /// Explain a diagnostic code from the error catalog.
    Explain {
        /// Diagnostic code (e.g. `name.undefined`).
        code: String,
    },
    /// Print project overview: entry, namespaces, imports.
    Summary {
        /// Path to the `.orl` entry file or project root.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Best-effort rewrite of pre-S3 Ori syntax (`.orl` files).
    ///
    /// Mechanical migrations: `namespace`→`module`, strip decl `func`,
    /// `import as`/`only`, `<>`→`[]`, `else if`→`elif`, `do(`→`(`, simple `?`→`try`,
    /// and scaffold headers for `implement`/`apply Trait to`. Does not rewrite `.oridoc`.
    #[command(name = "migrate-syntax")]
    MigrateSyntax {
        /// Files or directories to migrate (recurses for `*.orl`).
        paths: Vec<PathBuf>,
        /// Report changes without writing files.
        #[arg(long)]
        dry_run: bool,
        /// List every scanned file (`[ok]` for unchanged) and rewrite tags.
        #[arg(long, short = 'v')]
        verbose: bool,
    },
}

#[derive(Subcommand)]
enum EmitAction {
    /// Emit C source through the partial debug backend.
    C {
        /// Path to the `.orl` source file.
        file: PathBuf,
        /// Write generated C to this file instead of stdout.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum DocAction {
    /// Extract documentation from a source file (Markdown or HTML).
    File {
        /// Path to the `.orl` source file or project manifest.
        file: PathBuf,
        /// Output format (`markdown` default, or `html` for a static page).
        #[arg(long, value_enum, default_value_t = DocFormatCli::Markdown)]
        format: DocFormatCli,
        /// Write output to this file instead of stdout.
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
    /// Validate inline docs and `.oridoc` sidecar files.
    Check {
        /// Path to the `.orl` source file, `ori.proj`, or project root.
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Export stdlib + error catalog JSON for the documentation website.
    Export {
        /// Output JSON path (default: stdout).
        #[arg(short, long)]
        out: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, clap::ValueEnum)]
enum DocFormatCli {
    Markdown,
    Html,
}

fn main() {
    let cli = Cli::parse();
    let color = !cli.no_color && std::env::var("NO_COLOR").is_err();

    match &cli.command {
        Commands::New { path, name, lib } => {
            let kind = if *lib {
                pipeline::NewProjectKind::Lib
            } else {
                pipeline::NewProjectKind::App
            };
            match pipeline::run_new_project(
                path,
                pipeline::NewProjectOptions {
                    name: name.clone(),
                    kind,
                    is_init: false,
                },
            ) {
                Err(e) => {
                    eprintln!("ori: {}", e);
                    process::exit(2);
                }
                Ok(out) => {
                    eprintln!("project: {}", out.root.display());
                    eprintln!("manifest: {}", out.manifest.display());
                    eprintln!("entry: {}", out.entry.display());
                }
            }
        }

        Commands::Init { path, name, lib } => {
            let kind = if *lib {
                pipeline::NewProjectKind::Lib
            } else {
                pipeline::NewProjectKind::App
            };
            match pipeline::run_new_project(
                path,
                pipeline::NewProjectOptions {
                    name: name.clone(),
                    kind,
                    is_init: true,
                },
            ) {
                Err(e) => {
                    eprintln!("ori: {}", e);
                    process::exit(2);
                }
                Ok(out) => {
                    eprintln!("project: {}", out.root.display());
                    eprintln!("manifest: {}", out.manifest.display());
                    eprintln!("entry: {}", out.entry.display());
                }
            }
        }

        Commands::Check { file } => match pipeline::run_check(file) {
            Err(e) => {
                eprintln!("ori: {}", e);
                process::exit(2);
            }
            Ok(out) => {
                let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                let warnings = out.diagnostics.len() - errors;
                emit::render_all(&out.cache, &out.diagnostics, color);
                emit::print_summary(errors, warnings, color);
                process::exit(if out.has_errors { 1 } else { 0 });
            }
        },

        Commands::Doc { action } => match action {
            DocAction::File { file, format, out } => {
                let doc_format = match format {
                    DocFormatCli::Markdown => pipeline::DocFormat::Markdown,
                    DocFormatCli::Html => pipeline::DocFormat::Html,
                };
                match pipeline::run_doc_with_options(
                    file,
                    pipeline::DocOptions { format: doc_format },
                ) {
                    Err(e) => {
                        eprintln!("ori: {}", e);
                        process::exit(2);
                    }
                    Ok(doc) => {
                        let errors = doc.diagnostics.iter().filter(|d| d.is_error()).count();
                        let warnings = doc.diagnostics.len() - errors;
                        emit::render_all(&doc.cache, &doc.diagnostics, color);
                        emit::print_summary(errors, warnings, color);
                        if !doc.has_errors {
                            let content = match doc_format {
                                pipeline::DocFormat::Html => &doc.html,
                                pipeline::DocFormat::Markdown => &doc.markdown,
                            };
                            if let Some(path) = out {
                                std::fs::write(path, content).unwrap_or_else(|e| {
                                    eprintln!("ori: {}", e);
                                    process::exit(2);
                                });
                            } else {
                                print!("{content}");
                            }
                        }
                        process::exit(if doc.has_errors { 1 } else { 0 });
                    }
                }
            }
            DocAction::Check { path } => match pipeline::run_doc_check(path) {
                Err(e) => {
                    eprintln!("ori: {}", e);
                    process::exit(2);
                }
                Ok(out) => {
                    let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                    let warnings = out.diagnostics.len() - errors;
                    emit::render_all(&out.cache, &out.diagnostics, color);
                    emit::print_summary(errors, warnings, color);
                    process::exit(if out.has_errors { 1 } else { 0 });
                }
            },
            DocAction::Export { out } => {
                use ori_driver::doc_export;
                match out {
                    Some(path) => {
                        if let Err(e) = doc_export::write_doc_export(path) {
                            eprintln!("ori: {}", e);
                            process::exit(2);
                        }
                    }
                    None => match doc_export::export_doc_json() {
                        Err(e) => {
                            eprintln!("ori: {}", e);
                            process::exit(2);
                        }
                        Ok(json) => print!("{json}"),
                    },
                }
            }
        },

        Commands::Install { name, path, cache } => {
            match package::run_install_package(package::InstallPackageOptions {
                name: name.clone(),
                source: path.clone(),
                cache_root: cache.clone(),
            }) {
                Err(e) => {
                    eprintln!("ori: {}", e);
                    process::exit(2);
                }
                Ok(out) => {
                    eprintln!("cache: {}", out.cache_root.display());
                    for installed in out.packages {
                        let status = if installed.already_installed {
                            "already installed"
                        } else {
                            "installed"
                        };
                        eprintln!("  + {} v{} ({})", installed.name, installed.version, status);
                    }
                }
            }
        }

        Commands::Get { path, cache } => {
            match package::run_get_dependencies(package::GetDependenciesOptions {
                path: path.clone(),
                cache_root: cache.clone(),
            }) {
                Err(e) => {
                    eprintln!("ori: {}", e);
                    process::exit(2);
                }
                Ok(out) => {
                    eprintln!("cache: {}", out.cache_root.display());
                    if out.packages.is_empty() {
                        eprintln!("  (no dependencies to fetch)");
                    }
                    for installed in out.packages {
                        let status = if installed.already_installed {
                            "cached"
                        } else {
                            "fetched"
                        };
                        eprintln!("  + {} v{} ({})", installed.name, installed.version, status);
                    }
                }
            }
        }

        Commands::Publish {
            path,
            registry,
            token,
            force,
        } => match package::run_publish_package(package::PublishPackageOptions {
            path: path.clone(),
            registry: registry.clone(),
            token: token.clone(),
            force: *force,
        }) {
            Err(e) => {
                eprintln!("ori: {}", e);
                process::exit(2);
            }
            Ok(out) => {
                eprintln!(
                    "published {} v{} → {}",
                    out.name, out.version, out.location
                );
                eprintln!("registry: {}", out.registry);
            }
        }

        Commands::Test { file, filter } => match pipeline::run_test_with_options(
            file,
            pipeline::TestOptions {
                filter: filter.clone(),
            },
        ) {
            Err(e) => {
                eprintln!("ori: {}", e);
                process::exit(2);
            }
            Ok(out) => {
                let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                let warnings = out.diagnostics.len() - errors;
                emit::render_all(&out.cache, &out.diagnostics, color);
                emit::print_summary(errors, warnings, color);
                if !out.has_errors {
                    let skipped = out.results.iter().filter(|result| result.skipped).count();
                    let passed = out
                        .results
                        .iter()
                        .filter(|result| result.passed && !result.skipped)
                        .count();
                    let failed = out.results.iter().filter(|result| !result.passed).count();
                    for result in &out.results {
                        if result.skipped {
                            eprintln!("skip: {}", result.name);
                        } else if result.passed {
                            eprintln!("ok: {}", result.name);
                        } else {
                            eprintln!(
                                "fail: {} (status: {})",
                                result.name,
                                result
                                    .status
                                    .map(|status| status.to_string())
                                    .unwrap_or_else(|| "terminated".to_string())
                            );
                            if !result.stdout.trim().is_empty() {
                                eprintln!("stdout:\n{}", result.stdout.trim_end());
                            }
                            if !result.stderr.trim().is_empty() {
                                eprintln!("stderr:\n{}", result.stderr.trim_end());
                            }
                        }
                    }
                    let filter_note = out
                        .filter
                        .as_deref()
                        .map(|filter| format!(", filter `{filter}`"))
                        .unwrap_or_default();
                    if skipped > 0 {
                        eprintln!(
                            "tests: {passed} passed, {skipped} skipped, {failed} failed ({}/{} selected{filter_note})",
                            out.selected, out.discovered
                        );
                    } else {
                        eprintln!(
                            "tests: {passed} passed, {failed} failed ({}/{} selected{filter_note})",
                            out.selected, out.discovered
                        );
                    }
                }
                let failed = out.results.iter().any(|result| !result.passed);
                process::exit(if out.has_errors || failed { 1 } else { 0 });
            }
        },

        Commands::Fmt { file } => match pipeline::run_fmt(file) {
            Err(e) => {
                eprintln!("ori: {}", e);
                process::exit(2);
            }
            Ok(out) => {
                let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                let warnings = out.diagnostics.len() - errors;
                emit::render_all(&out.cache, &out.diagnostics, color);
                emit::print_summary(errors, warnings, color);
                if !out.has_errors {
                    print!("{}", out.formatted);
                }
                process::exit(if out.has_errors { 1 } else { 0 });
            }
        },

        Commands::Lex { file } => match pipeline::run_lex(file) {
            Err(e) => {
                eprintln!("ori: {}", e);
                process::exit(2);
            }
            Ok(out) => {
                for tok in &out.tokens {
                    println!("{:?}  {:?}", tok.kind, tok.span);
                }
                let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                emit::render_all(&out.cache, &out.diagnostics, color);
                process::exit(if errors > 0 { 1 } else { 0 });
            }
        },

        Commands::Compile {
            file,
            out,
            lib,
            native_raw,
        } => {
            let default_out = if *lib {
                pipeline::default_shared_lib_path(file)
            } else {
                file.with_extension(if cfg!(windows) { "exe" } else { "" })
            };
            let exe = out.as_deref().unwrap_or(&default_out);
            match pipeline::run_compile_with_options(
                file,
                exe,
                pipeline::CompileOptions {
                    native_raw: *native_raw,
                    lib: *lib,
                },
            ) {
                Err(e) => {
                    eprintln!("ori: {}", e);
                    process::exit(2);
                }
                Ok(out) => {
                    let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                    let warnings = out.diagnostics.len() - errors;
                    emit::render_all(&out.cache, &out.diagnostics, color);
                    emit::print_summary(errors, warnings, color);
                    if !out.has_errors {
                        let label = if *lib { "library" } else { "binary" };
                        eprintln!("{label}: {}", out.exe_path.display());
                    }
                    process::exit(if out.has_errors { 1 } else { 0 });
                }
            }
        }

        Commands::Run { file, native_raw } => {
            if pipeline::should_use_jit_for_run() {
                match pipeline::run_jit(file) {
                    Err(e) => {
                        eprintln!("ori: {}", e);
                        process::exit(2);
                    }
                    Ok(out) => {
                        let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                        let warnings = out.diagnostics.len() - errors;
                        emit::render_all(&out.cache, &out.diagnostics, color);
                        emit::print_summary(errors, warnings, color);
                        if out.has_errors {
                            process::exit(1);
                        }
                        process::exit(out.exit_code);
                    }
                }
            }
            let exe = temp_run_exe_path(file);
            let compile = pipeline::run_compile_with_options(
                file,
                &exe,
                pipeline::CompileOptions {
                    native_raw: *native_raw,
                    lib: false,
                },
            );
            match compile {
                Err(e) => {
                    let _ = std::fs::remove_file(&exe);
                    eprintln!("ori: {}", e);
                    process::exit(2);
                }
                Ok(out) => {
                    let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                    let warnings = out.diagnostics.len() - errors;
                    emit::render_all(&out.cache, &out.diagnostics, color);
                    emit::print_summary(errors, warnings, color);
                    if out.has_errors {
                        let _ = std::fs::remove_file(&out.exe_path);
                        process::exit(1);
                    }
                    let status = process::Command::new(&out.exe_path).status();
                    let _ = std::fs::remove_file(&out.exe_path);
                    match status {
                        Err(e) => {
                            eprintln!("ori: {}", e);
                            process::exit(2);
                        }
                        Ok(status) => process::exit(status.code().unwrap_or(1)),
                    }
                }
            }
        }

        Commands::Repl => {
            process::exit(run_repl(color));
        }

        Commands::Build {
            path,
            out,
            native_raw,
        } => {
            let default_out = default_build_exe_path(path);
            let exe = out.as_deref().unwrap_or(&default_out);
            match pipeline::run_build_native(
                path,
                exe,
                pipeline::CompileOptions {
                    native_raw: *native_raw,
                    lib: false,
                },
            ) {
                Err(e) => {
                    eprintln!("ori: {}", e);
                    process::exit(2);
                }
                Ok(out) => {
                    let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                    let warnings = out.diagnostics.len() - errors;
                    emit::render_all(&out.cache, &out.diagnostics, color);
                    emit::print_summary(errors, warnings, color);
                    if !out.has_errors {
                        eprintln!("binary: {}", out.exe_path.display());
                    }
                    process::exit(if out.has_errors { 1 } else { 0 });
                }
            }
        }

        Commands::Emit { action } => match action {
            EmitAction::C { file, out } => match pipeline::run_emit_c(file) {
                Err(e) => {
                    eprintln!("ori: {}", e);
                    process::exit(2);
                }
                Ok(build) => {
                    let errors = build.diagnostics.iter().filter(|d| d.is_error()).count();
                    let warnings = build.diagnostics.len() - errors;
                    emit::render_all(&build.cache, &build.diagnostics, color);
                    emit::print_summary(errors, warnings, color);
                    if !build.has_errors {
                        match out {
                            Some(p) => {
                                std::fs::write(p, &build.c_source).unwrap_or_else(|e| {
                                    eprintln!("ori: {}", e);
                                    process::exit(2);
                                });
                            }
                            None => print!("{}", build.c_source),
                        }
                    }
                    process::exit(if build.has_errors { 1 } else { 0 });
                }
            },
        },

        Commands::Doctor => {
            let report = pipeline::run_doctor();
            for check in &report.checks {
                let (icon, label) = match check.status {
                    pipeline::DoctorStatus::Ok => ("ok", "OK"),
                    pipeline::DoctorStatus::Warn => ("warn", "WARN"),
                    pipeline::DoctorStatus::Fail => ("fail", "FAIL"),
                };
                if color {
                    let color_code = match check.status {
                        pipeline::DoctorStatus::Ok => "\x1b[32m",
                        pipeline::DoctorStatus::Warn => "\x1b[33m",
                        pipeline::DoctorStatus::Fail => "\x1b[31m",
                    };
                    eprintln!(
                        "{color_code}[{label}]\x1b[0m {} — {}",
                        check.name, check.detail
                    );
                } else {
                    eprintln!("[{icon}] {} — {}", check.name, check.detail);
                }
            }
            process::exit(if report.has_failures() { 1 } else { 0 });
        }

        Commands::Update { check } => {
            let options = update::UpdateOptions {
                check_only: *check,
                exe_path: None,
            };
            match update::run_update(&options) {
                Ok(update::UpdateOutcome::UpToDate { current }) => {
                    println!("ori {current} is up to date");
                }
                Ok(update::UpdateOutcome::UpdateAvailable { current, latest }) => {
                    println!("ori {current} → {latest} available");
                    println!("run `ori update` to install it");
                }
                Ok(update::UpdateOutcome::Installed {
                    previous,
                    installed,
                    install_root,
                }) => {
                    println!(
                        "ori {previous} → {installed} installed in {}",
                        install_root.display()
                    );
                    println!("run `ori --version` to confirm");
                }
                Err(e) => {
                    eprintln!("ori: {}", e);
                    process::exit(1);
                }
            }
        }

        Commands::Explain { code } => match explain::explain_code(&code) {
            Some(entry) => {
                print!("{}", explain::format_explanation(entry));
            }
            None => {
                eprintln!("ori: unknown diagnostic code `{code}`");
                eprintln!("ori: see docs/spec/13-error-catalog.md for the full catalog");
                process::exit(2);
            }
        },

        Commands::Summary { path } => {
            let target = if path.as_os_str().is_empty() || path.as_path() == Path::new(".") {
                std::env::current_dir().unwrap_or_else(|e| {
                    eprintln!("ori: {}", e);
                    process::exit(2);
                })
            } else {
                path.clone()
            };
            let entry = if target.is_dir() {
                target.join("main.orl")
            } else {
                target
            };
            match pipeline::run_summary(&entry) {
                Err(e) => {
                    eprintln!("ori: {}", e);
                    process::exit(2);
                }
                Ok(summary) => {
                    print!("{}", pipeline::format_summary_text(&summary));
                }
            }
        }

        Commands::Parse { file } => match pipeline::run_parse(file) {
            Err(e) => {
                eprintln!("ori: {}", e);
                process::exit(2);
            }
            Ok(out) => {
                println!("{:#?}", out.ast);
                let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                emit::render_all(&out.cache, &out.diagnostics, color);
                process::exit(if errors > 0 { 1 } else { 0 });
            }
        },

        Commands::MigrateSyntax {
            paths,
            dry_run,
            verbose,
        } => {
            match pipeline::run_migrate_syntax(
                paths,
                pipeline::MigrateSyntaxOptions {
                    dry_run: *dry_run,
                    verbose: *verbose,
                },
            ) {
                Err(e) => {
                    eprintln!("ori: {e}");
                    process::exit(2);
                }
                Ok(report) => {
                    print!("{}", report.format_summary(*verbose));
                    if *dry_run {
                        eprintln!(
                            "migrate-syntax: dry-run only ({} file(s) would change)",
                            report.changed_count()
                        );
                    }
                    process::exit(0);
                }
            }
        }
    }
}

fn temp_run_exe_path(file: &std::path::Path) -> PathBuf {
    let stem = file
        .file_stem()
        .and_then(|name| name.to_str())
        .unwrap_or("app");
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    let mut name = format!("ori-run-{stem}-{}-{millis}", process::id());
    if cfg!(windows) {
        name.push_str(".exe");
    }
    std::env::temp_dir().join(name)
}

fn run_repl(color: bool) -> i32 {
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    let mut stderr = io::stderr();
    let mut imports = Vec::new();
    let mut bindings = Vec::new();

    let _ = writeln!(
        stderr,
        "Ori REPL. Use :quit to exit. Supports imports, const/var bindings, calls, and simple expressions."
    );
    let _ = write!(stderr, "ori> ");
    let _ = stderr.flush();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(line) => line,
            Err(e) => {
                let _ = writeln!(stderr, "ori: {e}");
                return 2;
            }
        };
        let trimmed = line.trim();
        if trimmed.is_empty() {
            let _ = write!(stderr, "ori> ");
            let _ = stderr.flush();
            continue;
        }
        if matches!(trimmed, ":quit" | ":exit" | "exit" | "quit") {
            return 0;
        }

        if trimmed.starts_with("import ") || trimmed.starts_with("public import ") {
            let mut next_imports = imports.clone();
            next_imports.push(trimmed.to_string());
            let source = repl_source(&next_imports, &bindings, None);
            match pipeline::run_check_source(&repl_temp_source_path(), source) {
                Ok(out) if !out.has_errors => imports = next_imports,
                Ok(out) => {
                    emit::render_all(&out.cache, &out.diagnostics, color);
                    emit::print_summary(
                        out.diagnostics.iter().filter(|d| d.is_error()).count(),
                        out.diagnostics.iter().filter(|d| !d.is_error()).count(),
                        color,
                    );
                }
                Err(e) => {
                    let _ = writeln!(stderr, "ori: {e}");
                }
            }
            let _ = write!(stderr, "ori> ");
            let _ = stderr.flush();
            continue;
        }

        if trimmed.starts_with("const ") || trimmed.starts_with("var ") {
            let mut next_bindings = bindings.clone();
            next_bindings.push(trimmed.to_string());
            let source = repl_source(&imports, &next_bindings, None);
            match pipeline::run_check_source(&repl_temp_source_path(), source) {
                Ok(out) if !out.has_errors => bindings = next_bindings,
                Ok(out) => {
                    emit::render_all(&out.cache, &out.diagnostics, color);
                    emit::print_summary(
                        out.diagnostics.iter().filter(|d| d.is_error()).count(),
                        out.diagnostics.iter().filter(|d| !d.is_error()).count(),
                        color,
                    );
                }
                Err(e) => {
                    let _ = writeln!(stderr, "ori: {e}");
                }
            }
            let _ = write!(stderr, "ori> ");
            let _ = stderr.flush();
            continue;
        }

        let statement = repl_statement_for(trimmed);
        let source = repl_source(&imports, &bindings, Some(&statement));
        let path = repl_temp_source_path();
        match std::fs::write(&path, source) {
            Err(e) => {
                let _ = writeln!(stderr, "ori: cannot write `{}`: {e}", path.display());
                return 2;
            }
            Ok(()) => match pipeline::run_jit(&path) {
                Err(e) => {
                    let _ = writeln!(stderr, "ori: {e}");
                }
                Ok(out) => {
                    let errors = out.diagnostics.iter().filter(|d| d.is_error()).count();
                    let warnings = out.diagnostics.len() - errors;
                    emit::render_all(&out.cache, &out.diagnostics, color);
                    emit::print_summary(errors, warnings, color);
                    let _ = stdout.flush();
                }
            },
        }
        let _ = std::fs::remove_file(&path);
        let _ = write!(stderr, "ori> ");
        let _ = stderr.flush();
    }

    0
}

fn repl_source(imports: &[String], bindings: &[String], statement: Option<&str>) -> String {
    // S3 surface: `module` header, no declaration `func`, import `path = alias`.
    let mut source = String::from("module repl.main\n\nimport ori.io = io\n");
    for import in imports {
        source.push_str(import);
        source.push('\n');
    }
    source.push_str("\nmain()\n");
    for binding in bindings {
        source.push_str("    ");
        source.push_str(binding);
        source.push('\n');
    }
    if let Some(statement) = statement {
        source.push_str("    ");
        source.push_str(statement);
        source.push('\n');
    }
    source.push_str("end\n");
    source
}

fn repl_statement_for(input: &str) -> String {
    if input.starts_with("io.")
        || input.starts_with("return ")
        || input.starts_with("using ")
        || input.starts_with("if ")
        || input.starts_with("match ")
        || input.starts_with("while ")
        || input.starts_with("for ")
    {
        input.to_string()
    } else if input.starts_with('"') || input.starts_with("f\"") {
        format!("io.println({input})")
    } else {
        format!("io.println(string({input}))")
    }
}

fn repl_temp_source_path() -> PathBuf {
    std::env::temp_dir().join(format!("ori-repl-{}.orl", process::id()))
}

fn default_build_exe_path(path: &Path) -> PathBuf {
    let exe_name = if cfg!(windows) { "app.exe" } else { "app" };
    if path.is_dir() {
        return path.join(exe_name);
    }
    if path.file_name().and_then(|name| name.to_str()) == Some("ori.proj") {
        return path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(exe_name);
    }
    path.with_extension(if cfg!(windows) { "exe" } else { "" })
}

#[cfg(test)]
mod tests {
    use super::Cli;
    use clap::CommandFactory;

    #[test]
    fn compile_help_declares_native_runtime_contract() {
        let mut command = Cli::command();
        let help = command.render_long_help().to_string();
        let compile_help = command
            .find_subcommand_mut("compile")
            .expect("compile subcommand should exist")
            .render_long_help()
            .to_string();
        let test_help = command
            .find_subcommand_mut("test")
            .expect("test subcommand should exist")
            .render_long_help()
            .to_string();
        let build_help = command
            .find_subcommand_mut("build")
            .expect("build subcommand should exist")
            .render_long_help()
            .to_string();
        let emit_help = command
            .find_subcommand_mut("emit")
            .expect("emit subcommand should exist")
            .render_long_help()
            .to_string();
        let run_help = command
            .find_subcommand_mut("run")
            .expect("run subcommand should exist")
            .render_long_help()
            .to_string();

        assert!(help.contains("packaged native runtime"), "{help}");
        assert!(!help.contains("requires `cc`"), "{help}");
        assert!(!help.contains("C compiler"), "{help}");
        assert!(
            compile_help.contains("packaged native runtime"),
            "{compile_help}"
        );
        assert!(compile_help.contains("--native-raw"), "{compile_help}");
        assert!(
            compile_help.contains("full native linker stdout/stderr"),
            "{compile_help}"
        );
        assert!(test_help.contains("native runtime"), "{test_help}");
        assert!(build_help.contains("native backend"), "{build_help}");
        assert!(build_help.contains("--native-raw"), "{build_help}");
        assert!(emit_help.contains("debug artifacts"), "{emit_help}");
        assert!(
            run_help.contains("Compile and run an Ori source file"),
            "{run_help}"
        );
        assert!(run_help.contains("--native-raw"), "{run_help}");
        assert!(
            help.contains("Extract documentation comments as Markdown"),
            "{help}"
        );
        assert!(help.contains("Run functions marked with `@test`"), "{help}");
        assert!(
            help.contains("Format an Ori source file and print the result"),
            "{help}"
        );
    }

    #[test]
    fn repl_wraps_simple_expression_for_printing() {
        assert_eq!(
            super::repl_statement_for("1 + 2"),
            "io.println(string(1 + 2))"
        );
        assert_eq!(super::repl_statement_for("\"ori\""), "io.println(\"ori\")");
        assert_eq!(
            super::repl_statement_for("io.println(\"ori\")"),
            "io.println(\"ori\")"
        );
    }
}
