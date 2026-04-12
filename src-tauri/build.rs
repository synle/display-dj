use std::process::Command;

fn main() {
    // Download the display-dj CLI sidecar binary if needed.
    // Pass the Cargo TARGET env var so the script downloads the correct
    // platform binary even when cross-compiling (e.g. x86_64 on an ARM host).
    let target = std::env::var("TARGET").unwrap_or_default();
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("download-sidecar.sh");
    let status = Command::new("bash")
        .arg(&script)
        .env("CARGO_BUILD_TARGET", &target)
        .status()
        .expect("failed to run download-sidecar.sh");
    if !status.success() {
        panic!("download-sidecar.sh failed with status: {}", status);
    }

    tauri_build::build()
}
