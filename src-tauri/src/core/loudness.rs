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

/// Try to switch Loudness Equalization on for one playback endpoint.
///
/// Only rewrites an existing value, so endpoints whose driver does not expose
/// the Microsoft enhancement are skipped. Blocking (spawns `reg.exe`); call
/// from a background thread. Errors are returned for logging only.
#[cfg(target_os = "windows")]
pub fn ensure_loudness_equalization(device_id: &str) -> Result<bool, String> {
    use super::win_cmd::hidden_command;
    let guid =
        endpoint_guid(device_id).ok_or_else(|| format!("unrecognized device id {device_id}"))?;
    let key = format!(
        r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\MMDevices\Audio\Render\{guid}\FxProperties"
    );
    let query = hidden_command("reg")
        .args(["query", &key, "/v", LOUDNESS_VALUE_NAME])
        .output()
        .map_err(|e| format!("reg query spawn failed: {e}"))?;
    if !query.status.success() {
        return Err("loudness equalization not supported on this endpoint".into());
    }
    let existing = parse_reg_binary(&String::from_utf8_lossy(&query.stdout))
        .ok_or("unparseable loudness value")?;
    let Some(next) = enabled_blob(&existing) else {
        return Ok(false);
    };
    let hex: String = next.iter().map(|b| format!("{b:02x}")).collect();
    let add = hidden_command("reg")
        .args([
            "add",
            &key,
            "/v",
            LOUDNESS_VALUE_NAME,
            "/t",
            "REG_BINARY",
            "/d",
            &hex,
            "/f",
        ])
        .output()
        .map_err(|e| format!("reg add spawn failed: {e}"))?;
    if !add.status.success() {
        return Err(format!(
            "reg add failed (admin rights likely required): {}",
            String::from_utf8_lossy(&add.stderr).trim()
        ));
    }
    Ok(true)
}

/// Non-Windows stub: loudness equalization is a Windows-only enhancement.
#[cfg(not(target_os = "windows"))]
pub fn ensure_loudness_equalization(_device_id: &str) -> Result<bool, String> {
    Ok(false)
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

    /// Unknown type tag or short blob is never rewritten.
    #[test]
    fn enabled_blob_rejects_unknown_layout() {
        assert_eq!(enabled_blob(&[0x13, 0, 0, 0, 1, 0, 0, 0, 0, 0]), None);
        assert_eq!(enabled_blob(&[0x0B, 0]), None);
    }
}
