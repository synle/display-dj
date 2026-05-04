use std::path::PathBuf;
use std::time::SystemTime;

fn main() {
    expose_app_version();
    tauri_build::build();
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
        // Build timestamp for dev builds: MM/DD/YYYY HH:MM
        let secs = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let days = secs / 86400;
        let (y, m, d) = civil_from_days(days as i64);
        let day_secs = secs % 86400;
        let hh = day_secs / 3600;
        let mm = (day_secs % 3600) / 60;
        format!("{version} [DEV - {m:02}/{d:02}/{y:04} {hh:02}:{mm:02}]")
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
