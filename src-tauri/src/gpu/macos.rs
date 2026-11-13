//! GPU utilization on macOS, read from the IORegistry.
//!
//! Every GPU driver publishes an `IOAccelerator` entry whose
//! `PerformanceStatistics` dictionary carries `Device Utilization %`. This is
//! the same number Activity Monitor's GPU history shows, and reading it needs
//! no elevated privileges.

use std::ffi::c_void;

use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef, CFMutableDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use io_kit_sys::types::{io_iterator_t, io_object_t};
use io_kit_sys::{
    IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperties,
    IOServiceGetMatchingServices, IOServiceMatching,
};

use crate::snapshot::clamp_pct;

const KERN_SUCCESS: i32 = 0;
/// `kIOMainPortDefault`; the named constant was renamed across SDK versions.
const MAIN_PORT_DEFAULT: u32 = 0;

/// Highest `Device Utilization %` across all GPUs, or `None` if no GPU
/// reports one.
pub fn sample() -> Option<f32> {
    let mut iterator: io_iterator_t = 0;
    // SAFETY: the matching dictionary is consumed by
    // IOServiceGetMatchingServices, and `iterator` is released below.
    let status = unsafe {
        let matching = IOServiceMatching(c"IOAccelerator".as_ptr());
        if matching.is_null() {
            return None;
        }
        IOServiceGetMatchingServices(MAIN_PORT_DEFAULT, matching as CFDictionaryRef, &mut iterator)
    };
    if status != KERN_SUCCESS {
        return None;
    }

    let mut highest: Option<f32> = None;
    loop {
        // SAFETY: `iterator` is a valid iterator until released.
        let entry = unsafe { IOIteratorNext(iterator) };
        if entry == 0 {
            break;
        }
        if let Some(v) = device_utilization(entry) {
            highest = Some(highest.map_or(v, |h| h.max(v)));
        }
        // SAFETY: `entry` came from IOIteratorNext and is released once.
        unsafe { IOObjectRelease(entry) };
    }
    // SAFETY: `iterator` came from IOServiceGetMatchingServices.
    unsafe { IOObjectRelease(iterator) };

    highest.map(clamp_pct)
}

fn device_utilization(entry: io_object_t) -> Option<f32> {
    let mut props: CFMutableDictionaryRef = std::ptr::null_mut();
    // SAFETY: on success `props` holds a +1 reference, which the wrapper
    // below takes ownership of.
    let status = unsafe {
        IORegistryEntryCreateCFProperties(entry, &mut props, std::ptr::null(), 0)
    };
    if status != KERN_SUCCESS || props.is_null() {
        return None;
    }
    // SAFETY: `props` is a non-null CFDictionary we own.
    let props: CFDictionary = unsafe { CFDictionary::wrap_under_create_rule(props as CFDictionaryRef) };

    let stats = lookup(&props, "PerformanceStatistics")?.downcast::<CFDictionary>()?;
    let value = lookup(&stats, "Device Utilization %")?.downcast::<CFNumber>()?;
    value.to_f64().map(|v| v as f32)
}

fn lookup(dict: &CFDictionary, key: &'static str) -> Option<CFType> {
    let key = CFString::from_static_string(key);
    let value = dict.find(key.as_concrete_TypeRef() as *const c_void)?;
    if value.is_null() {
        return None;
    }
    // SAFETY: the dictionary holds a live CF object at this key; the get rule
    // retains it so it outlives the dictionary borrow.
    Some(unsafe { CFType::wrap_under_get_rule(*value as *const _) })
}
