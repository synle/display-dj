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

    // Detect if this is a release build (tagged) or dev build.
    // Release builds (CI) set TAURI_RELEASE=true; local/dev builds don't.
    let is_release = std::env::var("TAURI_RELEASE").unwrap_or_default() == "true";

    let app_version = if is_release {
        version.to_string()
    } else {
        // Get short commit SHA for dev/beta builds
        let short_sha = Command::new("git")
            .args(["rev-parse", "--short", "HEAD"])
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        format!("{version} [beta - {short_sha}]")
    };

    println!("cargo:rustc-env=APP_VERSION={app_version}");
    // Expose build date as ISO 8601 (e.g. "2026-04-19") using SystemTime
    let build_date = {
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let days = secs / 86400;
        // Convert days since epoch to Y-M-D (simplified civil date calculation)
        let (y, m, d) = civil_from_days(days as i64);
        format!("{y:04}-{m:02}-{d:02}")
    };
    println!("cargo:rustc-env=BUILD_DATE={build_date}");
    // Expose build mode so debug log can use separate files
    println!(
        "cargo:rustc-env=IS_DEV_BUILD={}",
        if is_release { "false" } else { "true" }
    );
    // Re-run build script if tauri.conf.json changes or git HEAD moves
    println!("cargo:rerun-if-changed=tauri.conf.json");
    println!("cargo:rerun-if-changed=../.git/HEAD");
}

/// Ensure the sidecar binary exists for the current build target.
///
/// Tries to download the latest binary from GitHub releases first. If the
/// download fails (offline, network error, bad URL), falls back to the
/// committed binary in `src-tauri/binaries/`. The committed binaries act
/// as a safety net so builds never fail due to network issues.
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

    // Skip download if the binary already exists and is non-empty (avoids
    // re-downloading on every build, which triggers the file watcher in
    // `tauri dev` and causes an infinite rebuild loop).
    if output.exists() {
        let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
        if size > 0 {
            println!("Sidecar binary already exists ({size} bytes): {}", output.display());
            return;
        }
    }

    // Try downloading from GitHub releases first (gets the latest for this version)
    let version = sidecar_version();
    let url = format!("https://github.com/{REPO}/releases/download/{version}/{asset}");
    println!("Downloading display-dj {version} for {sidecar_name}...");

    let downloaded = Command::new("curl")
        .args(["-fSL", "--connect-timeout", "10", &url, "-o"])
        .arg(&output)
        .status()
        .ok()
        .map(|s| s.success())
        .unwrap_or(false);

    if downloaded {
        let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
        if size > 0 {
            println!("Downloaded: {} ({size} bytes)", output.display());
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
            return;
        }
        println!("Download produced empty file, falling back to committed binary");
    } else {
        println!("Download failed, falling back to committed binary");
    }

    // Fallback: use the committed binary already in the repo
    if output.exists() {
        let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
        if size > 0 {
            println!("Using committed sidecar binary ({size} bytes): {}", output.display());
            return;
        }
    }

    panic!("No sidecar binary available for {sidecar_name} — download failed and no committed binary found");
}

/// Convert days since Unix epoch to (year, month, day) civil date.
/// Algorithm from Howard Hinnant (public domain).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
