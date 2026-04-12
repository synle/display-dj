use std::process::Command;

fn main() {
    // Download the display-dj CLI sidecar binary if needed
    let script = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("download-sidecar.sh");
    let status = Command::new("bash")
        .arg(&script)
        .status()
        .expect("failed to run download-sidecar.sh");
    if !status.success() {
        panic!("download-sidecar.sh failed with status: {}", status);
    }

    tauri_build::build()
}
