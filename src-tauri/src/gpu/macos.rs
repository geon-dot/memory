//! GPU utilization on macOS, read from the IORegistry.
//!
//! Every GPU driver publishes an `IOAccelerator` entry whose
//! `PerformanceStatistics` dictionary carries `Device Utilization %`. This is
//! the same number Activity Monitor's GPU history shows, and reading it needs
//! no elevated privileges.
//!
//! Per process: on Apple Silicon each process that opens the GPU gets a user
//! client entry (`AGXDeviceUserClient`) under the accelerator. Its
//! `IOUserClientCreator` names the process (`"pid 176, WindowServer"`) and its
//! `AppUsage` array holds, per command queue, `accumulatedGPUTime`: GPU time
//! used so far in nanoseconds (checked against uptime through the sibling
//! `lastSubmittedTime`, and against `Device Utilization %` by sampling).
//! These entries are readable without elevated privileges too. Intel Macs'
//! GPU clients have no `AppUsage`, so there is no per-process reading there.

use std::ffi::c_void;

use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::dictionary::{CFDictionary, CFDictionaryRef, CFMutableDictionaryRef};
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use io_kit_sys::types::{io_iterator_t, io_object_t};
use io_kit_sys::{
    IOIteratorNext, IOObjectRelease, IORegistryEntryCreateCFProperties,
    IORegistryEntryGetChildIterator, IORegistryEntryGetRegistryEntryID,
    IOServiceGetMatchingServices, IOServiceMatching,
};

use super::{parse_creator, ClientTime};
use crate::snapshot::clamp_pct;

const KERN_SUCCESS: i32 = 0;
/// `kIOMainPortDefault`; the named constant was renamed across SDK versions.
const MAIN_PORT_DEFAULT: u32 = 0;

/// Highest `Device Utilization %` across all GPUs, or `None` if no GPU
/// reports one.
pub fn sample() -> Option<f32> {
    let mut highest: Option<f32> = None;
    for_each_accelerator(|entry| {
        if let Some(v) = properties(entry).and_then(|p| device_utilization(&p)) {
            highest = Some(highest.map_or(v, |h| h.max(v)));
        }
    })?;
    highest.map(clamp_pct)
}

/// GPU time so far of every GPU client that reports it, or `None` when no
/// client carries `AppUsage` (an Intel Mac) or the registry cannot be read.
pub fn client_times() -> Option<Vec<ClientTime>> {
    let mut clients = Vec::new();
    let mut any_usage = false;
    for_each_accelerator(|accelerator| {
        for_each_child(accelerator, |entry| {
            let Some(props) = properties(entry) else {
                return;
            };
            let Some(usage) = lookup(&props, "AppUsage").and_then(|u| u.downcast::<CFArray>())
            else {
                return;
            };
            any_usage = true;
            let Some((pid, name)) = lookup(&props, "IOUserClientCreator")
                .and_then(|c| c.downcast::<CFString>())
                .and_then(|c| parse_creator(&c.to_string()))
            else {
                return;
            };
            let gpu_ns = usage
                .iter()
                .filter_map(|item| {
                    // SAFETY: the array holds live CF objects; the get rule
                    // retains this one for the downcast below.
                    let item = unsafe { CFType::wrap_under_get_rule(*item as *const _) };
                    let dict = item.downcast::<CFDictionary>()?;
                    let time = lookup(&dict, "accumulatedGPUTime")?.downcast::<CFNumber>()?;
                    time.to_i64().and_then(|t| u64::try_from(t).ok())
                })
                .fold(0u64, u64::saturating_add);
            let mut id = 0u64;
            // SAFETY: `entry` is a live registry entry and `id` a valid out-pointer.
            if unsafe { IORegistryEntryGetRegistryEntryID(entry, &mut id) } != KERN_SUCCESS {
                return;
            }
            clients.push(ClientTime { id, pid, name, gpu_ns });
        });
    })?;
    any_usage.then_some(clients)
}

/// Calls `f` with every `IOAccelerator` service; `None` if they cannot be
/// listed. Each entry is released after `f` returns.
fn for_each_accelerator(mut f: impl FnMut(io_object_t)) -> Option<()> {
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
    drain(iterator, &mut f);
    Some(())
}

/// Calls `f` with every child of `entry` in the service plane. User clients
/// are children there, though they are never registered as services, so
/// matching by class does not find them.
fn for_each_child(entry: io_object_t, mut f: impl FnMut(io_object_t)) {
    let mut iterator: io_iterator_t = 0;
    // SAFETY: `entry` is live and `iterator` a valid out-pointer.
    let status =
        unsafe { IORegistryEntryGetChildIterator(entry, c"IOService".as_ptr(), &mut iterator) };
    if status == KERN_SUCCESS {
        drain(iterator, &mut f);
    }
}

/// Visits and releases every entry of `iterator`, then the iterator.
fn drain(iterator: io_iterator_t, f: &mut impl FnMut(io_object_t)) {
    loop {
        // SAFETY: `iterator` is a valid iterator until released.
        let entry = unsafe { IOIteratorNext(iterator) };
        if entry == 0 {
            break;
        }
        f(entry);
        // SAFETY: `entry` came from IOIteratorNext and is released once.
        unsafe { IOObjectRelease(entry) };
    }
    // SAFETY: `iterator` came from IOKit and is released once.
    unsafe { IOObjectRelease(iterator) };
}

/// All properties of a registry entry.
fn properties(entry: io_object_t) -> Option<CFDictionary> {
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
    Some(unsafe { CFDictionary::wrap_under_create_rule(props as CFDictionaryRef) })
}

fn device_utilization(props: &CFDictionary) -> Option<f32> {
    let stats = lookup(props, "PerformanceStatistics")?.downcast::<CFDictionary>()?;
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
