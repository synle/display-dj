use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

const REPO: &str = "synle/display-dj-cli";

fn main() {
    expose_app_version();
    download_sidecar();
    tauri_build::build();
}

/// Read the sidecar version. Prefers the `DISPLAY_DJ_CLI_VERSION` env var
/// (set by CI workflow_dispatch), falls back to `displayDjCliVersion` in package.json.
fn sidecar_version() -> String {
    if let Ok(v) = std::env::var("DISPLAY_DJ_CLI_VERSION") {
        if !v.is_empty() {
            return v;
        }
    }
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let pkg_path = manifest_dir.join("../package.json");
    let pkg: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&pkg_path).expect("failed to read package.json"),
    )
    .expect("failed to parse package.json");
    pkg["displayDjCliVersion"]
        .as_str()
        .expect("displayDjCliVersion missing in package.json")
        .to_string()
}

/// Read the version from tauri.conf.json (the single source of truth) and
/// expose it as the compile-time env var `APP_VERSION` so Rust code can use
/// `env!("APP_VERSION")` instead of `env!("CARGO_PKG_VERSION")`.
fn expose_app_version() {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let conf_path = manifest_dir.join("tauri.conf.json");
    let conf: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&conf_path).expect("failed to read tauri.conf.json"),
    )
    .expect("failed to parse tauri.conf.json");
    let version = conf["version"].as_str().expect("version missing in tauri.conf.json");
    println!("cargo:rustc-env=APP_VERSION={version}");
    // Re-run build script if tauri.conf.json changes
    println!("cargo:rerun-if-changed=tauri.conf.json");
}

/// Ensure the sidecar binary exists for the current build target.
///
/// Binaries are committed to the repo under `src-tauri/binaries/` for all
/// platforms. The download is only a fallback: it runs when the binary is
/// missing or has zero size (e.g. git-lfs placeholder or corrupt file).
/// This means offline builds and CI caching work out of the box.
fn download_sidecar() {
    let target = std::env::var("TARGET").expect("TARGET env var not set by Cargo");
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let binaries_dir = manifest_dir.join("binaries");
    std::fs::create_dir_all(&binaries_dir).expect("failed to create binaries dir");

    let (asset, sidecar_name) = match target.as_str() {
        "aarch64-apple-darwin" => ("display-dj-macos-arm64", "display-dj-server-aarch64-apple-darwin"),
        "x86_64-apple-darwin" => ("display-dj-macos-x64", "display-dj-server-x86_64-apple-darwin"),
        "x86_64-pc-windows-msvc" => ("display-dj-windows-x64.exe", "display-dj-server-x86_64-pc-windows-msvc.exe"),
        "aarch64-pc-windows-msvc" => ("display-dj-windows-arm64.exe", "display-dj-server-aarch64-pc-windows-msvc.exe"),
        "x86_64-unknown-linux-gnu" => ("display-dj-linux-x64", "display-dj-server-x86_64-unknown-linux-gnu"),
        "aarch64-unknown-linux-gnu" => ("display-dj-linux-arm64", "display-dj-server-aarch64-unknown-linux-gnu"),
        _ => panic!("Unsupported target triple: {target}"),
    };

    let output = binaries_dir.join(sidecar_name);

    // Use the committed binary if it exists and has non-zero size
    if output.exists() {
        let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
        if size > 0 {
            println!("Sidecar binary already exists ({size} bytes): {}", output.display());
            return;
        }
        println!("Sidecar binary exists but is empty, re-downloading: {}", output.display());
    }

    // Fallback: download from GitHub releases
    let version = sidecar_version();
    let url = format!("https://github.com/{REPO}/releases/download/{version}/{asset}");
    println!("Downloading display-dj {version} for {sidecar_name}...");

    let status = Command::new("curl")
        .args(["-fSL", &url, "-o"])
        .arg(&output)
        .status();

    match status {
        Ok(s) if s.success() => {
            // Verify the download produced a non-empty file
            let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
            if size == 0 {
                panic!("Downloaded sidecar is empty (0 bytes): {url}");
            }
            println!("Downloaded: {} ({size} bytes)", output.display());
        }
        Ok(_) => {
            panic!("curl failed to download {url} — check the version and network");
        }
        Err(e) => {
            panic!("failed to run curl: {e} — is curl installed?");
        }
    }

    // Make executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&output)
            .expect("failed to read sidecar metadata")
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&output, perms).expect("failed to chmod sidecar");
    }
}
