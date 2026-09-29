//! Best-effort "Loudness Equalization" enforcement for the active Windows
//! playback endpoint.
//!
//! Windows exposes no public API for audio enhancements. The Microsoft
//! enhancement APO stores its per-endpoint toggles as serialized PROPVARIANT
//! blobs under
//! `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{guid}\FxProperties`.
//! Loudness Equalization is property `{fc52a749-4be9-4510-896e-966ba6525980},3`
//! (a `VT_BOOL`). This key layout is undocumented and was inferred from
//! community tooling, so every step here is fire-and-forget: failures (missing
//! key, vendor APO without the property, access denied because the app runs
//! `asInvoker`) are logged and ignored.
//!
//! On non-Windows targets the entry point is a no-op.

/// Registry value name of the Loudness Equalization toggle inside `FxProperties`.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const LOUDNESS_VALUE_NAME: &str = "{fc52a749-4be9-4510-896e-966ba6525980},3";

/// PROPVARIANT type tag for `VT_BOOL`.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const VT_BOOL: u8 = 0x0B;

/// Byte offset of the 16-bit `VARIANT_BOOL` payload in the serialized blob.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const BOOL_PAYLOAD_OFFSET: usize = 8;

/// Extract the endpoint GUID from an `IMMDevice::GetId` string.
///
/// `"{0.0.0.00000000}.{abcd-...}"` → `Some("{abcd-...}")`. Returns `None` for
/// any id not ending in a braced segment.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn endpoint_guid(device_id: &str) -> Option<&str> {
    let guid = device_id.rsplit('.').next()?;
    (guid.len() > 2 && guid.starts_with('{') && guid.ends_with('}')).then_some(guid)
}

/// Parse the hex blob of a `reg query` line for [`LOUDNESS_VALUE_NAME`].
///
/// Returns the raw bytes, or `None` when the line is missing or malformed.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn parse_reg_binary(reg_output: &str) -> Option<Vec<u8>> {
    let line = reg_output
        .lines()
        .find(|l| l.trim_start().starts_with(LOUDNESS_VALUE_NAME))?;
    let hex = line.split_whitespace().last()?;
    if hex.len() % 2 != 0 {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
        .collect()
}

/// Decide the new blob for an existing Loudness Equalization value.
///
/// Returns `None` when no write is needed (already on) or the blob is not a
/// recognizable `VT_BOOL` PROPVARIANT (unknown layout → never write).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
pub(crate) fn enabled_blob(existing: &[u8]) -> Option<Vec<u8>> {
    if existing.len() < BOOL_PAYLOAD_OFFSET + 2 || existing[0] != VT_BOOL {
        return None;
    }
    let payload = &existing[BOOL_PAYLOAD_OFFSET..BOOL_PAYLOAD_OFFSET + 2];
    if payload == [0xFF, 0xFF] {
        return None;
    }
    let mut next = existing.to_vec();
    next[BOOL_PAYLOAD_OFFSET] = 0xFF;
    next[BOOL_PAYLOAD_OFFSET + 1] = 0xFF;
    Some(next)
}

/// Why a Loudness Equalization attempt did not change anything.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoudnessError {
    /// Endpoint has no Microsoft loudness value (expected; log at info).
    Unsupported(String),
    /// Real failure: spawn error, unparseable value, or rejected write (log at error).
    Failed(String),
}

impl std::fmt::Display for LoudnessError {
    /// Renders the inner diagnostic message.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(message) | Self::Failed(message) => f.write_str(message),
        }
    }
}

/// Serialized `VT_BOOL` PROPVARIANT set to `VARIANT_TRUE`, used when creating
/// the loudness value from scratch (community-documented layout, unverified
/// across drivers).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const LOUDNESS_ON_BLOB_HEX: &str = "0b00000001000000ffff0000";

/// Microsoft enhancement APO registration written by option 3 when an
/// endpoint has no `FxProperties` key at all. Pairs of (value name, CLSID):
/// PKEY_FX_PreMixEffectClsid / PostMixEffectClsid / UserInterfaceClsid.
/// CLSIDs come from community tooling (unverified); never written over an
/// existing key so vendor APOs are never replaced.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const MS_APO_REGISTRATION: [(&str, &str); 3] = [
    (
        "{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},1",
        "{62dc1a93-ae24-464c-a43e-452f824c4250}",
    ),
    (
        "{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},2",
        "{637c490d-eee3-4c0a-973f-371958802da2}",
    ),
    (
        "{d04e05a6-594b-4fb6-a80d-01af5eed7d1d},3",
        "{5860e1c5-f95c-4a7a-8ec8-8aef24f379a1}",
    ),
];

/// Longest reg output kept in one attempt log line.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const MAX_LOG_OUTPUT_CHARS: usize = 1500;

/// Outcome of every strategy tried for one endpoint.
///
/// `result` is the overall verdict; `attempts` holds one human-readable line
/// per option (`option N (name): ...`) so logs show which strategy worked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoudnessReport {
    /// `Ok(true)` written, `Ok(false)` already on, or why nothing changed.
    pub result: Result<bool, LoudnessError>,
    /// Ordered per-option diagnostics.
    pub attempts: Vec<String>,
}

/// Trim and bound command output for logging.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn clip(text: &[u8]) -> String {
    let text = String::from_utf8_lossy(text);
    let text = text.trim().replace("\r\n", " | ").replace('\n', " | ");
    text.chars().take(MAX_LOG_OUTPUT_CHARS).collect()
}

/// Try to switch Loudness Equalization on for one playback endpoint.
///
/// Runs ordered strategies and records each in [`LoudnessReport::attempts`]:
/// 0 inspect `FxProperties`; 1 rewrite an existing loudness value; 2 create the
/// value when the key exists; 3 register the Microsoft enhancement APO when the
/// key is missing; 4 restart `audiosrv` after any successful write. Writes to
/// HKLM need admin rights, so at `asInvoker` integrity options 2–4 usually log
/// "access denied". Blocking (spawns `reg.exe` / PowerShell); call from a
/// background thread.
#[cfg(target_os = "windows")]
pub fn ensure_loudness_equalization(device_id: &str) -> LoudnessReport {
    use super::win_cmd::hidden_command;
    use LoudnessError::{Failed, Unsupported};
    let mut attempts = Vec::new();
    let Some(guid) = endpoint_guid(device_id) else {
        return LoudnessReport {
            result: Err(Failed(format!("unrecognized device id {device_id}"))),
            attempts,
        };
    };
    let key = format!(
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{guid}\FxProperties"
    );
    /// Run `reg add <key> /v <name> /t <type> /d <data> /f`; Ok on exit 0.
    fn reg_add(key: &str, name: &str, kind: &str, data: &str) -> Result<(), String> {
        let out = super::win_cmd::hidden_command("reg")
            .args(["add", key, "/v", name, "/t", kind, "/d", data, "/f"])
            .output()
            .map_err(|e| format!("spawn failed: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "reg_exit={:?} stderr={:?}",
                out.status.code(),
                clip(&out.stderr)
            ))
        }
    }

    // Option 0: inspect what the endpoint already has.
    let listing = hidden_command("reg").args(["query", &key]).output();
    let key_exists = match &listing {
        Ok(out) if out.status.success() => {
            attempts.push(format!(
                "option 0 (inspect FxProperties): key present, values={:?}",
                clip(&out.stdout)
            ));
            true
        }
        Ok(out) => {
            attempts.push(format!(
                "option 0 (inspect FxProperties): key missing (reg_exit={:?} stderr={:?})",
                out.status.code(),
                clip(&out.stderr)
            ));
            false
        }
        Err(e) => {
            attempts.push(format!(
                "option 0 (inspect FxProperties): spawn failed: {e}"
            ));
            return LoudnessReport {
                result: Err(Failed(format!("reg query spawn failed: {e}"))),
                attempts,
            };
        }
    };
    let existing = listing
        .as_ref()
        .ok()
        .filter(|_| key_exists)
        .and_then(|out| parse_reg_binary(&String::from_utf8_lossy(&out.stdout)));

    let mut wrote = false;
    let mut last_error: Option<String> = None;
    if let Some(existing) = existing {
        // Option 1: flip the existing Microsoft loudness value.
        match enabled_blob(&existing) {
            None if existing.first() == Some(&VT_BOOL) => {
                attempts.push("option 1 (rewrite existing value): already on".into());
                return LoudnessReport {
                    result: Ok(false),
                    attempts,
                };
            }
            None => {
                let msg = format!(
                    "option 1 (rewrite existing value): unknown blob layout {existing:02x?}, not written"
                );
                attempts.push(msg.clone());
                last_error = Some(msg);
            }
            Some(next) => {
                let hex: String = next.iter().map(|b| format!("{b:02x}")).collect();
                match reg_add(&key, LOUDNESS_VALUE_NAME, "REG_BINARY", &hex) {
                    Ok(()) => {
                        attempts.push("option 1 (rewrite existing value): written".into());
                        wrote = true;
                    }
                    Err(e) => {
                        let msg = format!("option 1 (rewrite existing value): failed {e}");
                        attempts.push(msg.clone());
                        last_error = Some(msg);
                    }
                }
            }
        }
    } else {
        attempts.push("option 1 (rewrite existing value): skipped, value absent".into());
        if key_exists {
            // Option 2: key exists (driver has an FX chain) but no loudness value.
            match reg_add(
                &key,
                LOUDNESS_VALUE_NAME,
                "REG_BINARY",
                LOUDNESS_ON_BLOB_HEX,
            ) {
                Ok(()) => {
                    attempts.push("option 2 (create loudness value): written".into());
                    wrote = true;
                }
                Err(e) => {
                    let msg = format!("option 2 (create loudness value): failed {e}");
                    attempts.push(msg.clone());
                    last_error = Some(msg);
                }
            }
            attempts.push("option 3 (register Microsoft APO): skipped, key exists".into());
        } else {
            attempts.push("option 2 (create loudness value): skipped, key missing".into());
            // Option 3: no FX chain at all; register the Microsoft enhancement APO.
            let result = MS_APO_REGISTRATION
                .iter()
                .try_for_each(|(name, clsid)| reg_add(&key, name, "REG_SZ", clsid))
                .and_then(|()| {
                    reg_add(
                        &key,
                        LOUDNESS_VALUE_NAME,
                        "REG_BINARY",
                        LOUDNESS_ON_BLOB_HEX,
                    )
                });
            match result {
                Ok(()) => {
                    attempts.push("option 3 (register Microsoft APO): written".into());
                    wrote = true;
                }
                Err(e) => {
                    let msg = format!("option 3 (register Microsoft APO): failed {e}");
                    attempts.push(msg.clone());
                    last_error = Some(msg);
                }
            }
        }
    }

    if !wrote {
        attempts.push("option 4 (restart audiosrv): skipped, nothing written".into());
        let reason = last_error.unwrap_or_else(|| "no strategy applied".into());
        let error = if reason.contains("unknown blob layout") {
            Unsupported(reason)
        } else {
            Failed(reason)
        };
        return LoudnessReport {
            result: Err(error),
            attempts,
        };
    }
    // Option 4: the audio engine only reloads FX settings on service restart.
    match hidden_command("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "Restart-Service audiosrv -Force",
        ])
        .output()
    {
        Ok(out) if out.status.success() => {
            attempts.push("option 4 (restart audiosrv): restarted".into())
        }
        Ok(out) => attempts.push(format!(
            "option 4 (restart audiosrv): failed exit={:?} stderr={:?}",
            out.status.code(),
            clip(&out.stderr)
        )),
        Err(e) => attempts.push(format!("option 4 (restart audiosrv): spawn failed: {e}")),
    }
    LoudnessReport {
        result: Ok(true),
        attempts,
    }
}

/// Non-Windows stub: loudness equalization is a Windows-only enhancement.
#[cfg(not(target_os = "windows"))]
pub fn ensure_loudness_equalization(_device_id: &str) -> LoudnessReport {
    LoudnessReport {
        result: Ok(false),
        attempts: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Endpoint GUID is the trailing braced segment of an MMDevice id.
    #[test]
    fn endpoint_guid_extracts_trailing_segment() {
        assert_eq!(
            endpoint_guid("{0.0.0.00000000}.{1a2b3c4d-0000-1111-2222-333344445555}"),
            Some("{1a2b3c4d-0000-1111-2222-333344445555}")
        );
    }

    /// Non-MMDevice ids (macOS UID / pactl sink) yield no GUID.
    #[test]
    fn endpoint_guid_rejects_unbraced_id() {
        assert_eq!(
            endpoint_guid("alsa_output.pci-0000_00_1f.3.analog-stereo"),
            None
        );
    }

    /// A `reg query` REG_BINARY line parses to bytes.
    #[test]
    fn parse_reg_binary_reads_hex() {
        let out = "\r\nHKEY_LOCAL_MACHINE\\...\\FxProperties\r\n    {fc52a749-4be9-4510-896e-966ba6525980},3    REG_BINARY    0B000000010000000000000000\r\n";
        assert_eq!(
            parse_reg_binary(out),
            Some(vec![0x0B, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0])
        );
    }

    /// Output without the loudness value parses to None.
    #[test]
    fn parse_reg_binary_missing_value() {
        assert_eq!(parse_reg_binary("ERROR: not found"), None);
    }

    /// Disabled VT_BOOL flips its payload to VARIANT_TRUE and keeps other bytes.
    #[test]
    fn enabled_blob_flips_disabled_value() {
        let off = [0x0B, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0];
        assert_eq!(
            enabled_blob(&off),
            Some(vec![0x0B, 0, 0, 0, 1, 0, 0, 0, 0xFF, 0xFF, 0, 0])
        );
    }

    /// Already-enabled value needs no write.
    #[test]
    fn enabled_blob_skips_enabled_value() {
        assert_eq!(
            enabled_blob(&[0x0B, 0, 0, 0, 1, 0, 0, 0, 0xFF, 0xFF, 0, 0]),
            None
        );
    }

    /// Creation blob is an enabled VT_BOOL, so a later run treats it as already on.
    #[test]
    fn loudness_on_blob_is_enabled_vt_bool() {
        let bytes: Vec<u8> = (0..LOUDNESS_ON_BLOB_HEX.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&LOUDNESS_ON_BLOB_HEX[i..i + 2], 16).unwrap())
            .collect();
        assert_eq!(bytes, vec![0x0B, 0, 0, 0, 1, 0, 0, 0, 0xFF, 0xFF, 0, 0]);
        assert_eq!(enabled_blob(&bytes), None);
    }

    /// Log clipping joins lines and bounds length.
    #[test]
    fn clip_joins_lines_and_bounds_length() {
        assert_eq!(clip(b"  a\r\nb \n"), "a | b");
        assert_eq!(clip(&vec![b'x'; 5000]).len(), MAX_LOG_OUTPUT_CHARS);
    }

    /// Unknown type tag or short blob is never rewritten.
    #[test]
    fn enabled_blob_rejects_unknown_layout() {
        assert_eq!(enabled_blob(&[0x13, 0, 0, 0, 1, 0, 0, 0, 0, 0]), None);
        assert_eq!(enabled_blob(&[0x0B, 0]), None);
    }
}
