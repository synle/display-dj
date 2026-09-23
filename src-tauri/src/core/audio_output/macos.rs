//! macOS CoreAudio playback-endpoint support.

use super::{AudioOutputDevice, AudioOutputDeviceState, AudioOutputState};
use std::ffi::{c_char, c_void, CStr};
use std::mem::size_of;
use std::ptr;

type AudioObjectId = u32;
type AudioDeviceId = AudioObjectId;
type AudioObjectPropertySelector = u32;
type AudioObjectPropertyScope = u32;
type AudioObjectPropertyElement = u32;
type OsStatus = i32;
type CfIndex = i64;
type CfStringEncoding = u32;
type CfStringRef = *const c_void;

const AUDIO_SYSTEM_OBJECT: AudioObjectId = 1;
const MASTER_ELEMENT: AudioObjectPropertyElement = 0;
const UTF8_ENCODING: CfStringEncoding = 0x0800_0100;

const PROPERTY_DEVICES: AudioObjectPropertySelector = four_cc(*b"dev#");
const PROPERTY_DEFAULT_OUTPUT: AudioObjectPropertySelector = four_cc(*b"dOut");
const PROPERTY_DEFAULT_SYSTEM_OUTPUT: AudioObjectPropertySelector = four_cc(*b"sOut");
const PROPERTY_DEVICE_UID: AudioObjectPropertySelector = four_cc(*b"uid ");
const PROPERTY_STREAMS: AudioObjectPropertySelector = four_cc(*b"stm#");
const PROPERTY_NAME: AudioObjectPropertySelector = four_cc(*b"lnam");
const PROPERTY_DEVICE_IS_ALIVE: AudioObjectPropertySelector = four_cc(*b"livn");
const PROPERTY_DEVICE_CAN_BE_DEFAULT: AudioObjectPropertySelector = four_cc(*b"dflt");
const PROPERTY_TRANSPORT_TYPE: AudioObjectPropertySelector = four_cc(*b"tran");
const TRANSPORT_BUILT_IN: u32 = four_cc(*b"bltn");
const SCOPE_GLOBAL: AudioObjectPropertyScope = four_cc(*b"glob");
const SCOPE_OUTPUT: AudioObjectPropertyScope = four_cc(*b"outp");
/// Stable loopback endpoint UIDs that cannot route macOS system playback.
const UNSUPPORTED_OUTPUT_DEVICE_UIDS: &[&str] =
    &["MSLoopbackDriverDevice_UID", "zoom.us.zoomaudiodevice.001"];

#[repr(C)]
struct AudioObjectPropertyAddress {
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
    element: AudioObjectPropertyElement,
}

#[link(name = "CoreAudio", kind = "framework")]
extern "C" {
    fn AudioObjectGetPropertyDataSize(
        object_id: AudioObjectId,
        address: *const AudioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const c_void,
        data_size: *mut u32,
    ) -> OsStatus;
    fn AudioObjectGetPropertyData(
        object_id: AudioObjectId,
        address: *const AudioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const c_void,
        data_size: *mut u32,
        data: *mut c_void,
    ) -> OsStatus;
    fn AudioObjectSetPropertyData(
        object_id: AudioObjectId,
        address: *const AudioObjectPropertyAddress,
        qualifier_data_size: u32,
        qualifier_data: *const c_void,
        data_size: u32,
        data: *const c_void,
    ) -> OsStatus;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFStringGetLength(value: CfStringRef) -> CfIndex;
    fn CFStringGetMaximumSizeForEncoding(length: CfIndex, encoding: CfStringEncoding) -> CfIndex;
    fn CFStringGetCString(
        value: CfStringRef,
        buffer: *mut c_char,
        buffer_size: CfIndex,
        encoding: CfStringEncoding,
    ) -> bool;
    fn CFRelease(value: *const c_void);
}

/// Enumerates output-capable CoreAudio devices and the current default.
pub fn get_audio_output_state() -> Result<AudioOutputState, String> {
    let default_device = read_u32(AUDIO_SYSTEM_OBJECT, PROPERTY_DEFAULT_OUTPUT, SCOPE_GLOBAL).ok();
    let mut devices = Vec::new();

    for device_id in read_u32_list(AUDIO_SYSTEM_OBJECT, PROPERTY_DEVICES, SCOPE_GLOBAL)? {
        let id = read_cf_string(device_id, PROPERTY_DEVICE_UID, SCOPE_GLOBAL)?;
        if is_unsupported_output_uid(&id) {
            continue;
        }

        let has_output_streams = has_output_streams(device_id)?;
        let is_alive = read_u32(device_id, PROPERTY_DEVICE_IS_ALIVE, SCOPE_GLOBAL)? != 0;
        let can_be_default =
            read_u32(device_id, PROPERTY_DEVICE_CAN_BE_DEFAULT, SCOPE_OUTPUT)? != 0;
        if !should_list_output(has_output_streams, is_alive, can_be_default) {
            continue;
        }

        let name = read_cf_string(device_id, PROPERTY_NAME, SCOPE_GLOBAL)?;
        let is_built_in =
            read_u32(device_id, PROPERTY_TRANSPORT_TYPE, SCOPE_GLOBAL)? == TRANSPORT_BUILT_IN;
        devices.push(AudioOutputDevice {
            id,
            name: name.clone(),
            original_name: name,
            state: AudioOutputDeviceState::Enabled,
            is_built_in,
        });
    }

    let selected_device_id = default_device
        .and_then(|default_id| read_cf_string(default_id, PROPERTY_DEVICE_UID, SCOPE_GLOBAL).ok())
        .filter(|default_uid| devices.iter().any(|device| &device.id == default_uid));

    Ok(AudioOutputState {
        devices,
        selected_device_id,
    })
}

/// Changes the macOS default output device by stable CoreAudio UID.
pub fn set_audio_output_device(device_uid: &str) -> Result<(), String> {
    if is_unsupported_output_uid(device_uid) {
        return Err(format!(
            "unsupported audio output device cannot be selected: {}",
            device_uid
        ));
    }

    let device_id = read_u32_list(AUDIO_SYSTEM_OBJECT, PROPERTY_DEVICES, SCOPE_GLOBAL)?
        .into_iter()
        .find(|device_id| {
            read_cf_string(*device_id, PROPERTY_DEVICE_UID, SCOPE_GLOBAL)
                .map(|uid| uid == device_uid)
                .unwrap_or(false)
        })
        .ok_or_else(|| format!("audio output device disappeared: {}", device_uid))?;

    write_u32(
        AUDIO_SYSTEM_OBJECT,
        PROPERTY_DEFAULT_OUTPUT,
        SCOPE_GLOBAL,
        device_id,
    )?;

    if let Err(error) = write_u32(
        AUDIO_SYSTEM_OBJECT,
        PROPERTY_DEFAULT_SYSTEM_OUTPUT,
        SCOPE_GLOBAL,
        device_id,
    ) {
        log::warn!(
            "set_audio_output_device: default output changed, but system output did not: {}",
            error
        );
    }

    Ok(())
}

/// Converts a four-character CoreAudio selector into its numeric representation.
const fn four_cc(value: [u8; 4]) -> u32 {
    u32::from_be_bytes(value)
}

/// Builds a CoreAudio property address.
fn address(
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        selector,
        scope,
        element: MASTER_ELEMENT,
    }
}

/// Returns a CoreAudio property byte size with status validation.
fn property_data_size(
    object_id: AudioObjectId,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
) -> Result<u32, String> {
    let property_address = address(selector, scope);
    let mut data_size = 0;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(object_id, &property_address, 0, ptr::null(), &mut data_size)
    };
    check_status(status, "read CoreAudio property size")?;
    Ok(data_size)
}

/// Reads one `u32` CoreAudio property.
fn read_u32(
    object_id: AudioObjectId,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
) -> Result<u32, String> {
    let property_address = address(selector, scope);
    let mut value = 0;
    let mut data_size = size_of::<u32>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            object_id,
            &property_address,
            0,
            ptr::null(),
            &mut data_size,
            &mut value as *mut u32 as *mut c_void,
        )
    };
    check_status(status, "read CoreAudio u32 property")?;
    Ok(value)
}

/// Reads an array of `u32` CoreAudio property values.
fn read_u32_list(
    object_id: AudioObjectId,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
) -> Result<Vec<u32>, String> {
    let mut data_size = property_data_size(object_id, selector, scope)?;
    if data_size == 0 {
        return Ok(Vec::new());
    }
    if data_size as usize % size_of::<u32>() != 0 {
        return Err(format!(
            "CoreAudio property size {} is not aligned to u32",
            data_size
        ));
    }

    let mut values = vec![0; data_size as usize / size_of::<u32>()];
    let property_address = address(selector, scope);
    let status = unsafe {
        AudioObjectGetPropertyData(
            object_id,
            &property_address,
            0,
            ptr::null(),
            &mut data_size,
            values.as_mut_ptr() as *mut c_void,
        )
    };
    check_status(status, "read CoreAudio u32 list property")?;
    Ok(values)
}

/// Reads and owns a CoreFoundation string returned by CoreAudio.
fn read_cf_string(
    object_id: AudioObjectId,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
) -> Result<String, String> {
    let property_address = address(selector, scope);
    let mut value: CfStringRef = ptr::null();
    let mut data_size = size_of::<CfStringRef>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(
            object_id,
            &property_address,
            0,
            ptr::null(),
            &mut data_size,
            &mut value as *mut CfStringRef as *mut c_void,
        )
    };
    check_status(status, "read CoreAudio string property")?;
    if value.is_null() {
        return Err("CoreAudio returned a null string".into());
    }

    let result = unsafe {
        let length = CFStringGetLength(value);
        let max_bytes = CFStringGetMaximumSizeForEncoding(length, UTF8_ENCODING);
        if max_bytes < 0 {
            Err("CoreAudio returned an invalid string length".into())
        } else {
            let mut buffer = vec![0_i8; max_bytes as usize + 1];
            if CFStringGetCString(
                value,
                buffer.as_mut_ptr(),
                buffer.len() as CfIndex,
                UTF8_ENCODING,
            ) {
                Ok(CStr::from_ptr(buffer.as_ptr())
                    .to_string_lossy()
                    .into_owned())
            } else {
                Err("CoreAudio string was not valid UTF-8".into())
            }
        }
    };
    unsafe { CFRelease(value) };
    result
}

/// Checks whether a CoreAudio device exposes at least one output stream.
fn has_output_streams(device_id: AudioDeviceId) -> Result<bool, String> {
    Ok(property_data_size(device_id, PROPERTY_STREAMS, SCOPE_OUTPUT)? > 0)
}

/// Includes only live output devices CoreAudio allows as the default output.
fn should_list_output(has_output_streams: bool, is_alive: bool, can_be_default: bool) -> bool {
    has_output_streams && is_alive && can_be_default
}

/// Matches stable CoreAudio UIDs known to be non-playback loopback endpoints.
fn is_unsupported_output_uid(device_uid: &str) -> bool {
    UNSUPPORTED_OUTPUT_DEVICE_UIDS.contains(&device_uid)
}

/// Writes one `u32` CoreAudio property.
fn write_u32(
    object_id: AudioObjectId,
    selector: AudioObjectPropertySelector,
    scope: AudioObjectPropertyScope,
    value: u32,
) -> Result<(), String> {
    let property_address = address(selector, scope);
    let status = unsafe {
        AudioObjectSetPropertyData(
            object_id,
            &property_address,
            0,
            ptr::null(),
            size_of::<u32>() as u32,
            &value as *const u32 as *const c_void,
        )
    };
    check_status(status, "write CoreAudio property")
}

/// Converts a non-zero CoreAudio status into a useful error.
fn check_status(status: OsStatus, operation: &str) -> Result<(), String> {
    if status == 0 {
        Ok(())
    } else {
        Err(format!("{} failed with OSStatus {}", operation, status))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Four-character property selectors retain SDK byte ordering.
    #[test]
    fn four_cc_matches_core_audio_constants() {
        assert_eq!(four_cc(*b"dev#"), 0x6465_7623);
        assert_eq!(four_cc(*b"dOut"), 0x644f_7574);
    }

    /// Virtual loopback endpoints that cannot become defaults stay out of the UI.
    #[test]
    fn excludes_outputs_that_cannot_become_default() {
        assert!(should_list_output(true, true, true));
        assert!(!should_list_output(true, true, false));
        assert!(!should_list_output(true, false, true));
        assert!(!should_list_output(false, true, true));
    }

    /// Known conference-app loopback endpoints remain hidden and rejected.
    #[test]
    fn identifies_unsupported_output_device_uids() {
        assert!(is_unsupported_output_uid("MSLoopbackDriverDevice_UID"));
        assert!(is_unsupported_output_uid("zoom.us.zoomaudiodevice.001"));
        assert!(!is_unsupported_output_uid("BuiltInSpeakerDevice"));
        assert!(set_audio_output_device("MSLoopbackDriverDevice_UID")
            .unwrap_err()
            .contains("unsupported audio output device"));
    }

    /// Read-only enumeration never surfaces explicitly unsupported endpoints.
    #[test]
    fn enumerates_audio_outputs_without_mutation() {
        if let Ok(state) = get_audio_output_state() {
            assert!(state
                .devices
                .iter()
                .all(|device| !is_unsupported_output_uid(&device.id)));
        }
    }
}
