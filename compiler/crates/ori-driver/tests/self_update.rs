// End-to-end `ori update` against a local HTTP server and a fake packaged
// install — no real network, no real GitHub (LANG-CLI-1).
//
// Single #[test]: the flow drives process-global env vars
// (ORI_UPDATE_RELEASES_URL, ORI_TARGET_TRIPLE), and one test per process
// keeps them race-free (integration test files run as their own binary).

use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};

use ori_driver::update::{run_update, sha256_hex_of_file, UpdateOptions, UpdateOutcome};

const TEST_TARGET: &str = "x86_64-unknown-linux-gnu";
const PACKAGE_DIR_NAME: &str = "ori-x86_64-unknown-linux-gnu";

fn test_root(name: &str) -> PathBuf {
    let path =
        std::env::temp_dir().join(format!("ori_self_update_{}_{}", std::process::id(), name));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn write_file(path: &Path, contents: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, contents).unwrap();
}

/// Minimal HTTP/1.1 file server over an already-bound listener: replies to
/// each GET with the routed body, `Connection: close` per request. Runs
/// until the test process exits.
fn serve_routes(listener: TcpListener, routes: Vec<(&'static str, Vec<u8>)>) {
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let Ok(mut stream) = stream else { continue };
            let mut request = Vec::new();
            let mut chunk = [0u8; 1024];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                match stream.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => request.extend_from_slice(&chunk[..n]),
                }
            }
            let request_line = String::from_utf8_lossy(&request);
            let path = request_line
                .split_whitespace()
                .nth(1)
                .unwrap_or("/")
                .to_string();
            let response = match routes.iter().find(|(route, _)| *route == path) {
                Some((_, body)) => {
                    let mut response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    )
                    .into_bytes();
                    response.extend_from_slice(body);
                    response
                }
                None => b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    .to_vec(),
            };
            let _ = stream.write_all(&response);
        }
    });
}

fn make_fake_install(name: &str) -> PathBuf {
    let root = test_root(name);
    write_file(&root.join("ori"), "old-binary");
    write_file(
        &root
            .join("runtime")
            .join(TEST_TARGET)
            .join("runtime-link.json"),
        "{}",
    );
    write_file(&root.join("stdlib").join("old.orl"), "-- old stdlib");
    root
}

fn make_release_archive(workdir: &Path) -> PathBuf {
    let package_root = workdir.join(PACKAGE_DIR_NAME);
    write_file(&package_root.join("ori"), "new-binary");
    write_file(&package_root.join("ori-lsp"), "new-lsp");
    write_file(
        &package_root
            .join("runtime")
            .join(TEST_TARGET)
            .join("libori_runtime.a"),
        "new-runtime",
    );
    write_file(&package_root.join("stdlib").join("io.orl"), "-- new stdlib");
    write_file(
        &package_root.join("examples").join("hello.orl"),
        "-- example",
    );
    let archive = workdir.join("package.tar.gz");
    let status = std::process::Command::new("tar")
        .arg("-czf")
        .arg(&archive)
        .arg("-C")
        .arg(workdir)
        .arg(PACKAGE_DIR_NAME)
        .status()
        .expect("system tar not available");
    assert!(status.success(), "tar -czf failed");
    archive
}

fn release_json(version: &str, asset_url: &str, digest: &str) -> Vec<u8> {
    serde_json::json!([{
        "tag_name": format!("v{version}"),
        "prerelease": true,
        "assets": [{
            "name": format!("ori-v{version}-{TEST_TARGET}.tar.gz"),
            "browser_download_url": asset_url,
            "digest": format!("sha256:{digest}"),
        }],
    }])
    .to_string()
    .into_bytes()
}

fn point_updater_at(base_url: &str, route: &str) {
    std::env::set_var("ORI_UPDATE_RELEASES_URL", format!("{base_url}{route}"));
}

#[test]
fn update_flow_against_local_release_server() {
    std::env::set_var("ORI_TARGET_TRIPLE", TEST_TARGET);

    let workdir = test_root("workdir");
    let archive = make_release_archive(&workdir);
    let archive_sha256 = sha256_hex_of_file(&archive).unwrap();
    let archive_bytes = std::fs::read(&archive).unwrap();

    // Bind first: the release JSON embeds asset URLs pointing back at the
    // same server, so the port must be known before building the routes.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let server_url = format!("http://{}", listener.local_addr().unwrap());
    let package_url = format!("{server_url}/pkg");
    let routes = vec![
        (
            "/old.json",
            release_json("0.0.1", &package_url, &archive_sha256),
        ),
        (
            "/new.json",
            release_json("9.9.9", &package_url, &archive_sha256),
        ),
        (
            "/corrupt.json",
            release_json("8.8.8", &package_url, "deadbeef"),
        ),
        ("/pkg", archive_bytes),
    ];
    serve_routes(listener, routes);

    // Phase 1 — remote older than us: up to date, even with --check.
    point_updater_at(&server_url, "/old.json");
    let outcome = run_update(&UpdateOptions {
        check_only: true,
        exe_path: None,
    })
    .unwrap();
    assert!(
        matches!(outcome, UpdateOutcome::UpToDate { .. }),
        "expected UpToDate against an older release"
    );

    // Phase 2 — newer release + --check: reported, nothing touched.
    point_updater_at(&server_url, "/new.json");
    let outcome = run_update(&UpdateOptions {
        check_only: true,
        exe_path: None,
    })
    .unwrap();
    match outcome {
        UpdateOutcome::UpdateAvailable { current, latest } => {
            assert_eq!(current, env!("CARGO_PKG_VERSION"));
            assert_eq!(latest, "9.9.9");
        }
        _ => panic!("expected UpdateAvailable against a newer release"),
    }

    // Phase 3 — development layout (no runtime/ beside the exe) refuses.
    let dev_dir = test_root("dev_layout");
    let error = run_update(&UpdateOptions {
        check_only: false,
        exe_path: Some(dev_dir.join("target").join("ori")),
    })
    .unwrap_err();
    assert!(error.contains("update.unpackaged_install"), "{error}");

    // Phase 4 — checksum mismatch aborts before touching the install.
    point_updater_at(&server_url, "/corrupt.json");
    let corrupt_root = make_fake_install("corrupt_install");
    let error = run_update(&UpdateOptions {
        check_only: false,
        exe_path: Some(corrupt_root.join("ori")),
    })
    .unwrap_err();
    assert!(error.contains("update.checksum_mismatch"), "{error}");
    assert_eq!(
        std::fs::read_to_string(corrupt_root.join("ori")).unwrap(),
        "old-binary",
        "a failed update must leave the install untouched"
    );
    assert!(!corrupt_root.join(".ori-update-staging").exists());

    // Phase 5 — full update: download, verify, extract, swap.
    point_updater_at(&server_url, "/new.json");
    let install_root = make_fake_install("real_install");
    let outcome = run_update(&UpdateOptions {
        check_only: false,
        exe_path: Some(install_root.join("ori")),
    })
    .unwrap();
    match outcome {
        UpdateOutcome::Installed {
            previous,
            installed,
            install_root: reported_root,
        } => {
            assert_eq!(previous, env!("CARGO_PKG_VERSION"));
            assert_eq!(installed, "9.9.9");
            assert_eq!(reported_root, install_root);
        }
        _ => panic!("expected Installed"),
    }
    assert_eq!(
        std::fs::read_to_string(install_root.join("ori")).unwrap(),
        "new-binary"
    );
    assert_eq!(
        std::fs::read_to_string(install_root.join("ori-lsp")).unwrap(),
        "new-lsp"
    );
    assert_eq!(
        std::fs::read_to_string(
            install_root
                .join("runtime")
                .join(TEST_TARGET)
                .join("libori_runtime.a")
        )
        .unwrap(),
        "new-runtime"
    );
    assert!(install_root.join("stdlib").join("io.orl").is_file());
    assert!(
        !install_root.join("stdlib").join("old.orl").exists(),
        "replaced directories must not keep stale entries"
    );
    assert!(install_root.join("examples").join("hello.orl").is_file());
    assert!(!install_root.join(".ori-update-staging").exists());
    assert!(!install_root.join(".ori-update-backup").exists());

    std::env::remove_var("ORI_UPDATE_RELEASES_URL");
    std::env::remove_var("ORI_TARGET_TRIPLE");
}
