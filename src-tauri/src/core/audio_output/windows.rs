//! Windows Core Audio playback-endpoint support.

use super::{AudioOutputDevice, AudioOutputState};
use ::windows::core::{IUnknown, IUnknown_Vtbl, Interface, GUID, HRESULT, PCWSTR, PWSTR};
use ::windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use ::windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use ::windows::Win32::Media::Audio::{
    eCommunications, eConsole, eMultimedia, eRender, ERole, IMMDevice, IMMDeviceEnumerator,
    MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use ::windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;
use ::windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};
use std::ffi::c_void;
use std::ops::Deref;

const CLSID_POLICY_CONFIG_CLIENT: GUID = GUID::from_u128(0x870af99c_171d_4f9e_af0d_e63df40c2bc9);
const RPC_E_CHANGED_MODE: HRESULT = HRESULT(0x8001_0106_u32 as i32);

#[repr(transparent)]
#[derive(Clone, PartialEq, Eq)]
struct IPolicyConfig(IUnknown);

unsafe impl Interface for IPolicyConfig {
    type Vtable = IPolicyConfigVtable;
    const IID: GUID = GUID::from_u128(0x568b9108_44bf_40b4_9006_86afe5b5a620);
}

impl Deref for IPolicyConfig {
    type Target = IUnknown;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(C)]
struct IPolicyConfigVtable {
    base: IUnknown_Vtbl,
    get_mix_format: usize,
    get_device_format: usize,
    reset_device_format: usize,
    set_device_format: usize,
    get_processing_period: usize,
    set_processing_period: usize,
    get_share_mode: usize,
    set_share_mode: usize,
    get_property_value: usize,
    set_property_value: usize,
    set_default_endpoint: unsafe extern "system" fn(*mut c_void, PCWSTR, ERole) -> HRESULT,
    set_endpoint_visibility: usize,
}

/// Balances successful COM initialization for the current worker thread.
struct ComApartment {
    should_uninitialize: bool,
}

impl ComApartment {
    /// Initializes COM while tolerating an already-initialized different apartment.
    fn initialize() -> Result<Self, String> {
        let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        if result.is_ok() {
            Ok(Self {
                should_uninitialize: true,
            })
        } else if result == RPC_E_CHANGED_MODE {
            Ok(Self {
                should_uninitialize: false,
            })
        } else {
            Err(format!("initialize Windows COM failed: {}", result))
        }
    }
}

impl Drop for ComApartment {
    fn drop(&mut self) {
        if self.should_uninitialize {
            unsafe { CoUninitialize() };
        }
    }
}

/// Enumerates active Windows render endpoints and the current multimedia default.
pub fn get_audio_output_state() -> Result<AudioOutputState, String> {
    let _apartment = ComApartment::initialize()?;
    let enumerator = create_enumerator()?;
    let default_id = unsafe {
        enumerator
            .GetDefaultAudioEndpoint(eRender, eMultimedia)
            .ok()
            .and_then(|device| device_id(&device).ok())
    };
    let collection = unsafe {
        enumerator
            .EnumAudioEndpoints(eRender, DEVICE_STATE_ACTIVE)
            .map_err(|error| format!("enumerate Windows audio outputs failed: {}", error))?
    };
    let count = unsafe {
        collection
            .GetCount()
            .map_err(|error| format!("count Windows audio outputs failed: {}", error))?
    };
    let mut devices = Vec::with_capacity(count as usize);

    for index in 0..count {
        let device = unsafe {
            collection
                .Item(index)
                .map_err(|error| format!("read Windows audio output {} failed: {}", index, error))?
        };
        let id = device_id(&device)?;
        let name = device_name(&device).unwrap_or_else(|error| {
            log::warn!(
                "read Windows audio output name failed for {}: {}",
                id,
                error
            );
            id.clone()
        });
        devices.push(AudioOutputDevice {
            id,
            name: name.clone(),
            original_name: name,
        });
    }

    Ok(AudioOutputState {
        devices,
        selected_device_id: default_id,
    })
}

/// Changes all Windows default render roles through the private PolicyConfig API.
pub fn set_audio_output_device(device_id: &str) -> Result<(), String> {
    let _apartment = ComApartment::initialize()?;
    let policy: IPolicyConfig = unsafe {
        CoCreateInstance(&CLSID_POLICY_CONFIG_CLIENT, None, CLSCTX_ALL)
            .map_err(|error| format!("create Windows PolicyConfig client failed: {}", error))?
    };
    let wide_id: Vec<u16> = device_id.encode_utf16().chain(Some(0)).collect();
    let roles = [
        ("console", eConsole),
        ("multimedia", eMultimedia),
        ("communications", eCommunications),
    ];
    let mut failures = Vec::new();

    for (name, role) in roles {
        let result = unsafe {
            (policy.vtable().set_default_endpoint)(policy.as_raw(), PCWSTR(wide_id.as_ptr()), role)
        };
        if let Err(error) = result.ok() {
            failures.push(format!("{}: {}", name, error));
        }
    }

    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "set Windows default audio output failed for roles {}",
            failures.join(", ")
        ))
    }
}

/// Reads volume and mute state from the current multimedia render endpoint.
pub fn get_default_volume() -> Result<(u32, bool), String> {
    let _apartment = ComApartment::initialize()?;
    let endpoint = default_audio_device()?;
    let volume = endpoint_volume(&endpoint)?;
    let level = unsafe {
        volume
            .GetMasterVolumeLevelScalar()
            .map_err(|error| format!("read Windows endpoint volume failed: {}", error))?
    };
    let muted = unsafe {
        volume
            .GetMute()
            .map_err(|error| format!("read Windows endpoint mute state failed: {}", error))?
    };
    Ok((
        (level.clamp(0.0, 1.0) * 100.0).round() as u32,
        muted.as_bool(),
    ))
}

/// Sets volume on the current multimedia render endpoint.
pub fn set_default_volume(level: u16) -> Result<(), String> {
    let _apartment = ComApartment::initialize()?;
    let endpoint = default_audio_device()?;
    let volume = endpoint_volume(&endpoint)?;
    unsafe {
        volume
            .SetMasterVolumeLevelScalar(f32::from(level.min(100)) / 100.0, std::ptr::null())
            .map_err(|error| format!("set Windows endpoint volume failed: {}", error))
    }
}

/// Sets mute state on the current multimedia render endpoint.
pub fn set_default_mute(mute: bool) -> Result<(), String> {
    let _apartment = ComApartment::initialize()?;
    let endpoint = default_audio_device()?;
    let volume = endpoint_volume(&endpoint)?;
    unsafe {
        volume
            .SetMute(mute, std::ptr::null())
            .map_err(|error| format!("set Windows endpoint mute state failed: {}", error))
    }
}

/// Creates the public MMDevice endpoint enumerator.
fn create_enumerator() -> Result<IMMDeviceEnumerator, String> {
    unsafe {
        CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
            .map_err(|error| format!("create Windows audio enumerator failed: {}", error))
    }
}

/// Returns the current multimedia render endpoint.
fn default_audio_device() -> Result<IMMDevice, String> {
    let enumerator = create_enumerator()?;
    unsafe {
        enumerator
            .GetDefaultAudioEndpoint(eRender, eMultimedia)
            .map_err(|error| format!("read Windows default audio output failed: {}", error))
    }
}

/// Activates endpoint-volume control for an MMDevice.
fn endpoint_volume(device: &IMMDevice) -> Result<IAudioEndpointVolume, String> {
    unsafe {
        device
            .Activate(CLSCTX_ALL, None)
            .map_err(|error| format!("activate Windows endpoint volume failed: {}", error))
    }
}

/// Reads and frees the stable endpoint ID allocated by MMDevice.
fn device_id(device: &IMMDevice) -> Result<String, String> {
    let pointer = unsafe {
        device
            .GetId()
            .map_err(|error| format!("read Windows audio output id failed: {}", error))?
    };
    owned_wide_string(pointer, "Windows audio output id")
}

/// Reads the endpoint friendly name from its property store.
fn device_name(device: &IMMDevice) -> Result<String, String> {
    let store = unsafe {
        device
            .OpenPropertyStore(STGM_READ)
            .map_err(|error| format!("open Windows audio property store failed: {}", error))?
    };
    let value = unsafe {
        store.GetValue(&PKEY_Device_FriendlyName).map_err(|error| {
            format!(
                "read Windows audio friendly-name property failed: {}",
                error
            )
        })?
    };
    let pointer = unsafe {
        PropVariantToStringAlloc(&value)
            .map_err(|error| format!("convert Windows audio friendly name failed: {}", error))?
    };
    owned_wide_string(pointer, "Windows audio friendly name")
}

/// Converts and frees a COM task-allocated UTF-16 string.
fn owned_wide_string(pointer: PWSTR, description: &str) -> Result<String, String> {
    if pointer.is_null() {
        return Err(format!("{} was null", description));
    }
    let result = unsafe { pointer.to_string() }
        .map_err(|error| format!("{} was invalid UTF-16: {}", description, error));
    unsafe { CoTaskMemFree(Some(pointer.0 as *const c_void)) };
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// PolicyConfig role labels remain complete for all Windows default roles.
    #[test]
    fn windows_default_roles_are_distinct() {
        assert_ne!(eConsole, eMultimedia);
        assert_ne!(eMultimedia, eCommunications);
        assert_ne!(eConsole, eCommunications);
    }
}
