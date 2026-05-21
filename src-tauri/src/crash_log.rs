//! Crash logging — persists every Rust panic and (on macOS) every native crash
//! recorded by the OS to a single `crash.log` file in the app's config
//! directory. The file is plain text with `========== <KIND> ==========`
//! section markers so a support dump captures both:
//!
//! 1. **Rust panics** — installed via `std::panic::set_hook`. Captures
//!    timestamp, app version, OS+arch, thread, panic payload, source location,
//!    backtrace, the last ~80 lines of `debug.log`, and a snapshot of
//!    `preferences.json`.
//!
//! 2. **macOS native crashes** — at startup we scan
//!    `~/Library/Logs/DiagnosticReports/display-dj-*.ips` (the JSON-Lines
//!    crash reports macOS writes for any process abort/segfault) and append a
//!    summary of each one we haven't seen yet to the same `crash.log`. Tracks
//!    progress in `{config_dir}/.macos_crash_marker` so we don't re-import on
//!    every launch.
//!
//! The file rotates at ~2 MB by trimming from the head at an entry boundary so
//! the most recent entries are always preserved.
//!
//! Unlike `debug.log`, crash logging is **NOT gated on `debug_logging`** —
//! crash data is always written. Crashes are rare and the diagnostic value is
//! high; users who want a fully silent app can delete the file.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

/// Returns the absolute path to the crash log file.
/// Lives next to `debug.log` / `preferences.json` in the app config dir so a
/// support dump (Open App Folder) picks it up automatically.
pub fn crash_log_path() -> PathBuf {
    crate::config::config_dir().join("crash.log")
}

/// Rotate the log when it crosses this size. Half is dropped from the head on
/// rotation so callers don't have to think about it.
const MAX_CRASH_LOG_SIZE: u64 = 2 * 1024 * 1024;

/// Trim the log to ~50% from the most-recent end, splitting on the next entry
/// boundary so we never produce a half-truncated record. No-op when under the
/// size limit.
fn rotate_if_needed() {
    let path = crash_log_path();
    let Ok(meta) = std::fs::metadata(&path) else {
        return;
    };
    if meta.len() <= MAX_CRASH_LOG_SIZE {
        return;
    }
    let Ok(content) = std::fs::read_to_string(&path) else {
        return;
    };
    let half = content.len() / 2;
    let from = content[half..]
        .find("==========")
        .map(|i| half + i)
        .unwrap_or(half);
    let _ = std::fs::write(&path, &content[from..]);
}

/// Append a chunk of text to `crash.log`, creating it if missing and rotating
/// first if oversized. All failure modes (disk full, permission denied) are
/// silently ignored — we never want crash-logging to itself panic.
fn append(text: &str) {
    rotate_if_needed();
    let path = crash_log_path();
    use std::io::Write;
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
    {
        let _ = f.write_all(text.as_bytes());
    }
}

/// Return the last `n` lines of `debug.log` (or `<unavailable>` on read
/// failure). Used to embed pre-crash context inside each panic record so the
/// reader doesn't have to cross-reference two files.
fn debug_log_tail(n: usize) -> String {
    let path = crate::config::debug_log_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            let mut last: Vec<&str> = content.lines().rev().take(n).collect();
            last.reverse();
            last.join("\n")
        }
        Err(_) => "<unavailable>".into(),
    }
}

/// Read `preferences.json` verbatim and return it (or `<unavailable>`) for
/// embedding inside a crash record. No secrets live in preferences (it's just
/// brightness curves + tiling config + keybindings) so the raw JSON is fine.
fn preferences_snapshot() -> String {
    let path = crate::config::config_dir().join("preferences.json");
    std::fs::read_to_string(&path).unwrap_or_else(|_| "<unavailable>".into())
}

/// Install the `std::panic::set_hook` that writes every Rust panic to
/// `crash.log`. Idempotent — safe to call multiple times; only the first
/// invocation installs the hook. Also force-enables `RUST_BACKTRACE=1` so the
/// captured backtrace is non-empty when symbols are available.
///
/// The hook chains to the previously-installed hook (e.g. env_logger's default
/// stderr print) so we don't lose existing behavior — we add to it.
pub fn install_panic_hook() {
    static INSTALLED: AtomicBool = AtomicBool::new(false);
    if INSTALLED.swap(true, Ordering::SeqCst) {
        return;
    }
    // Force backtraces on so `Backtrace::force_capture()` returns useful output
    // even when the user didn't set RUST_BACKTRACE in their environment. Bundled
    // GUI apps inherit no shell env, so this is the only place that gets set.
    // SAFETY: set_var is only unsafe in multi-threaded environments per the
    // Rust 1.85+ deprecation; we set this before any work spawns threads.
    std::env::set_var("RUST_BACKTRACE", "1");

    let prev = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let entry = format_panic_entry(info);
        append(&entry);
        // Chain to the previous hook so stderr output / env_logger still fires.
        prev(info);
    }));
}

/// Format a single panic record as a self-contained text block, including the
/// section header + footer markers used by `rotate_if_needed` to find safe
/// split points.
fn format_panic_entry(info: &std::panic::PanicHookInfo) -> String {
    let timestamp = chrono::Local::now().format("%Y-%m-%dT%H:%M:%S%.3f%:z");
    let version = env!("APP_VERSION");
    let build_date = env!("BUILD_DATE");
    let is_dev = env!("IS_DEV_BUILD");
    let thread = std::thread::current();
    let thread_name = thread.name().unwrap_or("<unnamed>").to_string();
    let location = info
        .location()
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "<unknown>".into());
    let payload = panic_payload_str(info);
    let backtrace = std::backtrace::Backtrace::force_capture();
    let tail = debug_log_tail(80);
    let prefs = preferences_snapshot();

    format!(
        "\n========== RUST PANIC ==========\n\
         timestamp: {timestamp}\n\
         app_version: {version}\n\
         build_date: {build_date}\n\
         is_dev_build: {is_dev}\n\
         os: {os} {arch}\n\
         thread: {thread_name}\n\
         location: {location}\n\
         payload: {payload}\n\
         \n\
         backtrace:\n{backtrace}\n\
         \n\
         recent debug.log tail (last 80 lines):\n\
         {tail}\n\
         \n\
         preferences snapshot:\n\
         {prefs}\n\
         ========== END RUST PANIC ==========\n",
        os = std::env::consts::OS,
        arch = std::env::consts::ARCH,
    )
}

/// Best-effort downcast of the panic payload to a string. `panic!()` with
/// `&str` and `String` are the common cases; anything else (e.g. a custom
/// payload type) falls back to a marker.
fn panic_payload_str(info: &std::panic::PanicHookInfo) -> String {
    let payload = info.payload();
    if let Some(s) = payload.downcast_ref::<&str>() {
        (*s).to_string()
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s.clone()
    } else {
        "<non-string panic payload>".into()
    }
}

/// On macOS, scan `~/Library/Logs/DiagnosticReports/display-dj-*.ips` for
/// crash reports we haven't seen yet and append a summary of each to
/// `crash.log`. Stateless on non-macOS platforms.
///
/// Tracks progress via `{config_dir}/.macos_crash_marker` (just stores the
/// Unix mtime of the newest already-imported report). On every launch we only
/// import reports with a newer mtime, so a long-running install never
/// re-imports the same crash. Safe to call multiple times.
#[cfg(target_os = "macos")]
pub fn import_macos_native_crashes() {
    let Some(home) = std::env::var_os("HOME") else {
        return;
    };
    let reports_dir = PathBuf::from(home).join("Library/Logs/DiagnosticReports");
    let Ok(entries) = std::fs::read_dir(&reports_dir) else {
        return;
    };

    let marker_path = crate::config::config_dir().join(".macos_crash_marker");
    let last_imported_secs: u64 = std::fs::read_to_string(&marker_path)
        .ok()
        .and_then(|s| s.trim().parse().ok())
        .unwrap_or(0);

    let mut to_import: Vec<(u64, PathBuf)> = Vec::new();
    let mut newest = last_imported_secs;

    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        if !name.starts_with("display-dj-") || !name.ends_with(".ips") {
            continue;
        }
        let Ok(meta) = entry.metadata() else { continue };
        let Ok(mtime) = meta.modified() else { continue };
        let secs = mtime
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        if secs <= last_imported_secs {
            continue;
        }
        if secs > newest {
            newest = secs;
        }
        to_import.push((secs, path));
    }

    to_import.sort_by_key(|(s, _)| *s);
    let imported = to_import.len();
    for (_, path) in to_import {
        if let Some(summary) = summarize_ips(&path) {
            append(&summary);
        }
    }

    if newest > last_imported_secs {
        let _ = std::fs::write(&marker_path, newest.to_string());
    }
    if imported > 0 {
        log::info!("crash_log: imported {} macOS native crash report(s)", imported);
    }
}

/// No-op on non-macOS. Keeps the call-site in `lib.rs::run()` platform-free.
#[cfg(not(target_os = "macos"))]
pub fn import_macos_native_crashes() {}

/// Parse a single `.ips` file (JSON Lines: 1-line header + 1-line body) and
/// return a human-readable summary block for `crash.log`, or `None` if the
/// file is malformed.
///
/// The summary intentionally collapses the verbose `.ips` schema into the
/// fields that actually matter for triage: exception type, signal, faulting
/// thread name, and the top ~40 stack frames of that thread. Other threads,
/// images, register state, etc. live in the original `.ips` if anyone needs
/// them.
#[cfg(target_os = "macos")]
fn summarize_ips(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    // `.ips` is JSON-Lines: first line is the metadata header, second line is
    // the full crash body. Anything past line 2 is ignored by Apple's tooling.
    let mut parts = content.splitn(2, '\n');
    let header_line = parts.next()?;
    let body_line = parts.next()?;
    let header: serde_json::Value = serde_json::from_str(header_line).ok()?;
    let body: serde_json::Value = serde_json::from_str(body_line).ok()?;

    let app_version = header
        .get("app_version")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let bundle_version = header
        .get("build_version")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let timestamp = header
        .get("timestamp")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let incident = header
        .get("incident_id")
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let bug_type = header.get("bug_type").and_then(|v| v.as_str()).unwrap_or("?");

    let os_train = body
        .get("osVersion")
        .and_then(|v| v.get("train"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let os_build = body
        .get("osVersion")
        .and_then(|v| v.get("build"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let cpu = body.get("cpuType").and_then(|v| v.as_str()).unwrap_or("?");
    let proc_name = body.get("procName").and_then(|v| v.as_str()).unwrap_or("?");
    let parent_proc = body
        .get("parentProc")
        .and_then(|v| v.as_str())
        .unwrap_or("?");

    let exception_type = body
        .get("exception")
        .and_then(|e| e.get("type"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let signal = body
        .get("exception")
        .and_then(|e| e.get("signal"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let termination = body
        .get("termination")
        .and_then(|t| t.get("indicator"))
        .and_then(|v| v.as_str())
        .unwrap_or("?");
    let asi = body
        .get("asi")
        .map(|v| v.to_string())
        .unwrap_or_else(|| "{}".into());

    let faulting_idx = body
        .get("faultingThread")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as usize;

    let threads = body.get("threads").and_then(|v| v.as_array());
    let (thread_name, frames_str) = match threads.and_then(|ts| ts.get(faulting_idx)) {
        Some(t) => {
            let name = t.get("name").and_then(|v| v.as_str()).unwrap_or("<unnamed>");
            let mut out = String::new();
            if let Some(frames) = t.get("frames").and_then(|f| f.as_array()) {
                for (i, frame) in frames.iter().enumerate().take(40) {
                    let symbol = frame
                        .get("symbol")
                        .and_then(|v| v.as_str())
                        .unwrap_or("<unsymbolicated>");
                    let sym_off = frame
                        .get("symbolLocation")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let img_idx = frame
                        .get("imageIndex")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    let img_off = frame
                        .get("imageOffset")
                        .and_then(|v| v.as_u64())
                        .unwrap_or(0);
                    out.push_str(&format!(
                        "  {i:2}: {symbol} +{sym_off}  (imageIndex={img_idx}, imageOffset={img_off})\n"
                    ));
                }
            }
            (name.to_string(), out)
        }
        None => ("<missing>".to_string(), String::new()),
    };

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?");

    Some(format!(
        "\n========== MACOS NATIVE CRASH ==========\n\
         source_file: {filename}\n\
         incident_id: {incident}\n\
         bug_type: {bug_type}\n\
         timestamp: {timestamp}\n\
         crashed_app_version: {app_version}\n\
         crashed_bundle_version: {bundle_version}\n\
         proc: {proc_name}  parent: {parent_proc}\n\
         os: macOS {os_train} ({os_build})  cpu: {cpu}\n\
         exception_type: {exception_type}\n\
         signal: {signal}\n\
         termination: {termination}\n\
         asi: {asi}\n\
         faulting_thread: '{thread_name}' (idx={faulting_idx})\n\
         frames (top 40):\n{frames_str}\
         ========== END MACOS NATIVE CRASH ==========\n",
    ))
}

/// Tauri command: open `crash.log` in the user's default editor / viewer. If
/// the file doesn't exist yet, return `Ok(())` quietly so the UI button doesn't
/// fail on a fresh install.
#[tauri::command]
pub fn open_crash_log() -> Result<(), String> {
    let path = crash_log_path();
    if !path.exists() {
        // Create an empty file so opening it always succeeds — users can paste
        // a crash later without rerunning the app.
        let _ = std::fs::write(&path, "");
    }
    open::that(&path).map_err(|e| format!("failed to open crash log: {e}"))
}

/// Tauri command: return the full contents of `crash.log` (or empty string if
/// none). Used by the About panel to show the most recent crash inline.
#[tauri::command]
pub fn get_crash_log() -> String {
    std::fs::read_to_string(crash_log_path()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: point `DISPLAY_DJ_CONFIG_DIR` at a fresh tempdir for the duration
    /// of the test. Returns the temp PathBuf so the test can clean it up.
    fn with_temp_config_dir<F: FnOnce(&PathBuf)>(f: F) {
        let _guard = crate::config::TEST_CONFIG_DIR_LOCK.lock().unwrap();
        let dir = std::env::temp_dir().join(format!(
            "display-dj-crashlog-test-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("DISPLAY_DJ_CONFIG_DIR", &dir);
        f(&dir);
        std::env::remove_var("DISPLAY_DJ_CONFIG_DIR");
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// `append` should create the file on first call and concatenate on
    /// subsequent calls so `crash.log` is an append-only newest-last log.
    #[test]
    fn append_creates_and_concatenates() {
        with_temp_config_dir(|_| {
            append("first\n");
            append("second\n");
            let content = std::fs::read_to_string(crash_log_path()).unwrap();
            assert_eq!(content, "first\nsecond\n");
        });
    }

    /// `rotate_if_needed` is a no-op when the file is under the size limit —
    /// otherwise we'd churn through disk on every small write.
    #[test]
    fn rotate_is_noop_when_small() {
        with_temp_config_dir(|_| {
            std::fs::write(crash_log_path(), b"small").unwrap();
            rotate_if_needed();
            assert_eq!(std::fs::read(crash_log_path()).unwrap(), b"small");
        });
    }

    /// When the file is over the limit, rotation must trim at an entry
    /// boundary (`==========`) so we never produce a half-record at the top
    /// of the rotated file. Verifies the post-rotation file is shorter AND
    /// starts at a real entry marker.
    #[test]
    fn rotate_trims_at_entry_boundary() {
        with_temp_config_dir(|_| {
            // Build a file ~3 MB with three sections so the trim happens
            // strictly inside the file.
            let chunk = format!(
                "{}\n",
                "x".repeat((MAX_CRASH_LOG_SIZE / 3) as usize)
            );
            let content = format!(
                "========== RUST PANIC ==========\n{chunk}\n\
                 ========== RUST PANIC ==========\n{chunk}\n\
                 ========== RUST PANIC ==========\n{chunk}\n",
            );
            std::fs::write(crash_log_path(), &content).unwrap();
            rotate_if_needed();
            let after = std::fs::read_to_string(crash_log_path()).unwrap();
            assert!(after.len() < content.len(), "file should shrink");
            assert!(
                after.starts_with("=========="),
                "rotated file must start at an entry marker, got: {:?}",
                &after[..40.min(after.len())]
            );
        });
    }

    /// `debug_log_tail` returns `<unavailable>` when no `debug.log` exists, so
    /// the panic record always has SOMETHING in that slot instead of a blank
    /// line that looks like the read silently succeeded.
    #[test]
    fn debug_log_tail_missing_file() {
        with_temp_config_dir(|_| {
            assert_eq!(debug_log_tail(10), "<unavailable>");
        });
    }

    /// `debug_log_tail` returns only the last N lines, in original order.
    /// Verifies both bounds — `n` smaller than line count clips, and the
    /// returned order matches file order (not reversed).
    #[test]
    fn debug_log_tail_returns_last_n() {
        with_temp_config_dir(|_| {
            let path = crate::config::debug_log_path();
            std::fs::write(&path, "a\nb\nc\nd\ne\n").unwrap();
            assert_eq!(debug_log_tail(2), "d\ne");
        });
    }

    /// `preferences_snapshot` falls back gracefully when the file is missing
    /// (fresh-install state) so the panic record can still be written.
    #[test]
    fn preferences_snapshot_missing() {
        with_temp_config_dir(|_| {
            assert_eq!(preferences_snapshot(), "<unavailable>");
        });
    }

    /// `preferences_snapshot` returns the file contents verbatim when present.
    /// We deliberately don't try to parse/redact — preferences contain no
    /// secrets and the raw JSON is the most useful diagnostic.
    #[test]
    fn preferences_snapshot_present() {
        with_temp_config_dir(|dir| {
            let p = dir.join("preferences.json");
            std::fs::write(&p, r#"{"foo":1}"#).unwrap();
            assert_eq!(preferences_snapshot(), r#"{"foo":1}"#);
        });
    }

    /// `get_crash_log` returns empty string on a fresh install, not a panic or
    /// an error — the About panel needs a benign default.
    #[test]
    fn get_crash_log_missing_returns_empty() {
        with_temp_config_dir(|_| {
            assert_eq!(get_crash_log(), "");
        });
    }

    /// `get_crash_log` returns the file contents when present so the About
    /// panel can render the latest entry inline.
    #[test]
    fn get_crash_log_returns_contents() {
        with_temp_config_dir(|_| {
            append("crash entry\n");
            assert_eq!(get_crash_log(), "crash entry\n");
        });
    }

    /// `format_panic_entry` must include the version, OS marker, and the
    /// section header/footer so rotation and humans can both find boundaries.
    /// We trigger a real panic via `catch_unwind` so the `PanicHookInfo` is
    /// produced by the runtime, not synthesized.
    #[test]
    fn format_panic_entry_includes_required_fields() {
        with_temp_config_dir(|_| {
            // We can't construct a `PanicHookInfo` directly — it's only
            // produced by the panic runtime — so we install a one-shot hook
            // that captures the formatted entry and then trigger a panic.
            use std::sync::Mutex;
            static CAPTURED: Mutex<Option<String>> = Mutex::new(None);
            let prev = std::panic::take_hook();
            std::panic::set_hook(Box::new(|info| {
                let mut guard = CAPTURED.lock().unwrap();
                *guard = Some(format_panic_entry(info));
            }));
            let _ = std::panic::catch_unwind(|| panic!("test-payload-marker"));
            std::panic::set_hook(prev);

            let entry = CAPTURED.lock().unwrap().take().expect("hook should have fired");
            assert!(entry.contains("========== RUST PANIC =========="));
            assert!(entry.contains("========== END RUST PANIC =========="));
            assert!(entry.contains("test-payload-marker"));
            assert!(entry.contains("app_version:"));
            assert!(entry.contains("os: "));
            assert!(entry.contains("backtrace:"));
        });
    }

    /// `summarize_ips` extracts the headline triage fields (signal, faulting
    /// thread, top frames) from a real-shape .ips fixture. This is the
    /// contract the macOS importer relies on — if the parse breaks, every
    /// imported crash becomes a blank record.
    #[cfg(target_os = "macos")]
    #[test]
    fn summarize_ips_extracts_key_fields() {
        with_temp_config_dir(|dir| {
            let ips = dir.join("fixture.ips");
            // Minimal but realistic .ips: 1-line header + 1-line body.
            let header = r#"{"app_name":"display-dj","app_version":"7.0.26","timestamp":"2026-05-20 19:50:16.00 -0700","incident_id":"ABC","bug_type":"309"}"#;
            let body = r#"{"osVersion":{"train":"macOS 26.5","build":"25F71"},"cpuType":"ARM-64","procName":"display-dj","parentProc":"launchd","exception":{"type":"EXC_CRASH","signal":"SIGABRT"},"termination":{"indicator":"Abort trap: 6"},"asi":{"libsystem_c.dylib":["abort() called"]},"faultingThread":0,"threads":[{"name":"main","frames":[{"symbol":"__pthread_kill","symbolLocation":8,"imageIndex":4,"imageOffset":38376},{"symbol":"abort","symbolLocation":148,"imageIndex":6,"imageOffset":493124}]}]}"#;
            std::fs::write(&ips, format!("{header}\n{body}")).unwrap();
            let summary = summarize_ips(&ips).expect("should parse");
            assert!(summary.contains("MACOS NATIVE CRASH"));
            assert!(summary.contains("crashed_app_version: 7.0.26"));
            assert!(summary.contains("signal: SIGABRT"));
            assert!(summary.contains("EXC_CRASH"));
            assert!(summary.contains("faulting_thread: 'main'"));
            assert!(summary.contains("__pthread_kill"));
            assert!(summary.contains("abort() called"));
        });
    }

    /// Malformed `.ips` (e.g. truncated JSON) must return `None` instead of
    /// panicking — otherwise the importer would crash on a crash file.
    #[cfg(target_os = "macos")]
    #[test]
    fn summarize_ips_malformed_returns_none() {
        with_temp_config_dir(|dir| {
            let ips = dir.join("bad.ips");
            std::fs::write(&ips, "not-json").unwrap();
            assert!(summarize_ips(&ips).is_none());
        });
    }
}
