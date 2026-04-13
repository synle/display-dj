use std::path::PathBuf;
use std::process::Command;
use std::time::SystemTime;

const VERSION: &str = "v0.4.1";
const REPO: &str = "synle/display-dj-cli";

fn main() {
    download_sidecar();
    tauri_build::build();
}

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

    // Skip download if binary already exists and is less than 1 day old
    if output.exists() {
        if let Ok(metadata) = output.metadata() {
            if let Ok(modified) = metadata.modified() {
                let age = SystemTime::now().duration_since(modified).unwrap_or_default();
                if age.as_secs() < 86400 {
                    println!("Sidecar binary already exists and is recent: {}", output.display());
                    return;
                }
            }
        }
    }

    let url = format!("https://github.com/{REPO}/releases/download/{VERSION}/{asset}");
    println!("Downloading display-dj {VERSION} for {sidecar_name}...");

    let status = Command::new("curl")
        .args(["-fSL", &url, "-o"])
        .arg(&output)
        .status()
        .expect("failed to run curl — is curl installed?");

    if !status.success() {
        panic!("curl failed to download {url}");
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

    let size = std::fs::metadata(&output).map(|m| m.len()).unwrap_or(0);
    println!("Downloaded: {} ({size} bytes)", output.display());
}
