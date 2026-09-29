//! Best-effort "Loudness Equalization" enforcement for the active Windows
//! playback endpoint.
//!
//! Windows exposes no public API for audio enhancements. The Microsoft
//! enhancement APO stores its per-endpoint toggles as serialized PROPVARIANT
//! blobs under
//! `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{guid}\FxProperties`.
//! Loudness Equalization is property `{fc52a749-4be9-4510-896e-966ba6525980},3`
//! (a `VT_BOOL`). This key layout is undocumented and was inferred from
//! community tooling. HKLM here is writable only by the audio service, so the
//! primary path asks that service to write it via the private PolicyConfig
//! `SetPropertyValue` (the Sound control panel's path); a direct registry write
//! is the fallback. Every step is fire-and-forget and logged per option.
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
    /// Every write path was refused by Windows permissions (log at warn).
    Denied(String),
    /// Real failure: spawn error, unparseable value, or rejected write (log at error).
    Failed(String),
}

impl std::fmt::Display for LoudnessError {
    /// Renders the inner diagnostic message.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Unsupported(message) | Self::Denied(message) | Self::Failed(message) => f.write_str(message),
        }
    }
}

/// Serialized `VT_BOOL` PROPVARIANT set to `VARIANT_TRUE`, used when creating
/// the loudness value from scratch (community-documented layout, unverified
/// across drivers).
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const LOUDNESS_ON_BLOB_HEX: &str = "0b00000001000000ffff0000";

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

/// Loudness Equalization property set id (`fmtid` of [`LOUDNESS_VALUE_NAME`]).
#[cfg(target_os = "windows")]
const LOUDNESS_FMTID: windows::core::GUID =
    windows::core::GUID::from_u128(0xfc52a749_4be9_4510_896e_966ba6525980);

/// Loudness Equalization property id (`pid` of [`LOUDNESS_VALUE_NAME`]).
#[cfg(target_os = "windows")]
const LOUDNESS_PID: u32 = 3;

/// True when `reg.exe` stderr reports a permissions failure.
#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn is_access_denied(text: &str) -> bool {
    text.to_ascii_lowercase().contains("access is denied")
}

/// Try to switch Loudness Equalization on for one playback endpoint.
///
/// Ordered strategies, each recorded in [`LoudnessReport::attempts`]:
/// 0 dump `FxProperties` (and subkeys) read-only; 1 write the property through
/// the audio service with PolicyConfig `SetPropertyValue` and read it back
/// (works where HKLM is locked even for Administrators); 2 registry fallback:
/// create the `FxProperties` key, then create or flip the loudness value;
/// 3 restart `audiosrv` only after a registry write. Blocking; call from a
/// background thread.
#[cfg(target_os = "windows")]
pub fn ensure_loudness_equalization(device_id: &str) -> LoudnessReport {
    use super::win_cmd::hidden_command;
    use LoudnessError::{Denied, Failed};
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

    // Option 0: read-only dump of the endpoint's FX store, including subkeys.
    match hidden_command("reg").args(["query", &key, "/s"]).output() {
        Ok(out) if out.status.success() => attempts.push(format!(
            "option 0 (inspect FxProperties): values={:?}",
            clip(&out.stdout)
        )),
        Ok(out) => attempts.push(format!(
            "option 0 (inspect FxProperties): key missing (reg_exit={:?} stderr={:?})",
            out.status.code(),
            clip(&out.stderr)
        )),
        Err(e) => attempts.push(format!("option 0 (inspect FxProperties): spawn failed: {e}")),
    }

    // Option 1: let the audio service write the property (Sound panel path).
    let policy_error = match crate::core::audio_output::get_fx_bool_property(
        device_id,
        LOUDNESS_FMTID,
        LOUDNESS_PID,
    ) {
        Ok(Some(true)) => {
            attempts.push("option 1 (PolicyConfig SetPropertyValue): already on".into());
            return LoudnessReport { result: Ok(false), attempts };
        }
        Ok(current) => match crate::core::audio_output::set_fx_bool_property(
            device_id,
            LOUDNESS_FMTID,
            LOUDNESS_PID,
            true,
        ) {
            Ok(true) => {
                attempts.push(format!(
                    "option 1 (PolicyConfig SetPropertyValue): written (was {current:?}, read back true)"
                ));
                return LoudnessReport { result: Ok(true), attempts };
            }
            Ok(false) => format!(
                "option 1 (PolicyConfig SetPropertyValue): write accepted but read back false (was {current:?})"
            ),
            Err(e) => format!("option 1 (PolicyConfig SetPropertyValue): failed {e}"),
        },
        Err(e) => format!("option 1 (PolicyConfig SetPropertyValue): read failed {e}"),
    };
    attempts.push(policy_error.clone());

    // Option 2: registry fallback. Create the key first, then the value.
    /// Run `reg add` with extra args; Ok on exit 0, else exit code + stderr.
    fn reg_add(args: &[&str]) -> Result<(), String> {
        let out = super::win_cmd::hidden_command("reg")
            .arg("add")
            .args(args)
            .arg("/f")
            .output()
            .map_err(|e| format!("spawn failed: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("reg_exit={:?} stderr={:?}", out.status.code(), clip(&out.stderr)))
        }
    }
    let registry = reg_add(&[key.as_str()])
        .map_err(|e| format!("create key {e}"))
        .and_then(|()| {
            let query = hidden_command("reg")
                .args(["query", &key, "/v", LOUDNESS_VALUE_NAME])
                .output()
                .map_err(|e| format!("query spawn failed: {e}"))?;
            let existing = query
                .status
                .success()
                .then(|| parse_reg_binary(&String::from_utf8_lossy(&query.stdout)))
                .flatten();
            let hex: String = match existing {
                Some(bytes) if bytes.first() == Some(&VT_BOOL) => match enabled_blob(&bytes) {
                    None => return Ok(false),
                    Some(next) => next.iter().map(|b| format!("{b:02x}")).collect(),
                },
                Some(bytes) => return Err(format!("unknown blob layout {bytes:02x?}, not written")),
                None => LOUDNESS_ON_BLOB_HEX.to_string(),
            };
            reg_add(&[key.as_str(), "/v", LOUDNESS_VALUE_NAME, "/t", "REG_BINARY", "/d", &hex])
                .map(|()| true)
                .map_err(|e| format!("set value {e}"))
        });
    let wrote = match registry {
        Ok(false) => {
            attempts.push("option 2 (registry: create key, then value): already on".into());
            attempts.push("option 3 (restart audiosrv): skipped, nothing written".into());
            return LoudnessReport { result: Ok(false), attempts };
        }
        Ok(true) => {
            attempts.push("option 2 (registry: create key, then value): written".into());
            true
        }
        Err(e) => {
            let msg = format!("option 2 (registry: create key, then value): failed {e}");
            attempts.push(msg.clone());
            attempts.push("option 3 (restart audiosrv): skipped, nothing written".into());
            let reason = format!("{policy_error}; {msg}");
            let error = if is_access_denied(&msg) { Denied(reason) } else { Failed(reason) };
            return LoudnessReport { result: Err(error), attempts };
        }
    };

    // Option 3: the audio engine reloads registry FX settings only on restart.
    debug_assert!(wrote);
    match hidden_command("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", "Restart-Service audiosrv -Force"])
        .output()
    {
        Ok(out) if out.status.success() => {
            attempts.push("option 3 (restart audiosrv): restarted".into())
        }
        Ok(out) => attempts.push(format!(
            "option 3 (restart audiosrv): failed exit={:?} stderr={:?}",
            out.status.code(),
            clip(&out.stderr)
        )),
        Err(e) => attempts.push(format!("option 3 (restart audiosrv): spawn failed: {e}")),
    }
    LoudnessReport { result: Ok(true), attempts }
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

    /// reg.exe permission failures are recognized case-insensitively.
    #[test]
    fn access_denied_is_detected() {
        assert!(is_access_denied("stderr=\"ERROR: Access is denied.\""));
        assert!(!is_access_denied("ERROR: The system was unable to find the specified registry key"));
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
