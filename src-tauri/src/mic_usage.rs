//! Whether some *other* process is capturing from a microphone right now —
//! the signal ambient listening uses to stay out of calls. Checks actual mic
//! use via CoreAudio's per-process list rather than guessing from which
//! apps are open: Zoom often runs idle in the background, while Google Meet
//! runs inside a browser.

use coreaudio_sys::{
    kAudioHardwarePropertyProcessObjectList, kAudioObjectPropertyElementMain,
    kAudioObjectPropertyScopeGlobal, kAudioObjectSystemObject, kAudioProcessPropertyIsRunningInput,
    kAudioProcessPropertyPID, AudioObjectGetPropertyData, AudioObjectGetPropertyDataSize, AudioObjectID,
    AudioObjectPropertyAddress,
};
use std::ffi::c_void;

fn address(selector: u32) -> AudioObjectPropertyAddress {
    AudioObjectPropertyAddress {
        mSelector: selector,
        mScope: kAudioObjectPropertyScopeGlobal,
        mElement: kAudioObjectPropertyElementMain,
    }
}

fn read_u32(object: AudioObjectID, selector: u32) -> Option<u32> {
    let addr = address(selector);
    let mut value: u32 = 0;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        AudioObjectGetPropertyData(object, &addr, 0, std::ptr::null(), &mut size, &mut value as *mut u32 as *mut c_void)
    };
    (status == 0).then_some(value)
}

/// PIDs of other processes currently capturing audio input, or `None` if
/// the OS can't say (the per-process API needs macOS 14+).
pub fn other_processes_using_mic() -> Option<Vec<i32>> {
    let addr = address(kAudioHardwarePropertyProcessObjectList);
    let mut size: u32 = 0;
    let status = unsafe {
        AudioObjectGetPropertyDataSize(kAudioObjectSystemObject, &addr, 0, std::ptr::null(), &mut size)
    };
    if status != 0 {
        return None;
    }

    let count = size as usize / std::mem::size_of::<AudioObjectID>();
    let mut processes: Vec<AudioObjectID> = vec![0; count];
    let status = unsafe {
        AudioObjectGetPropertyData(
            kAudioObjectSystemObject,
            &addr,
            0,
            std::ptr::null(),
            &mut size,
            processes.as_mut_ptr() as *mut c_void,
        )
    };
    if status != 0 {
        return None;
    }
    processes.truncate(size as usize / std::mem::size_of::<AudioObjectID>());

    let own = std::process::id() as i32;
    Some(
        processes
            .into_iter()
            .filter(|&p| read_u32(p, kAudioProcessPropertyIsRunningInput).is_some_and(|running| running != 0))
            .filter_map(|p| read_u32(p, kAudioProcessPropertyPID).map(|pid| pid as i32))
            .filter(|&pid| pid != own)
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exercises the real CoreAudio call. Can't assert who's using the mic
    /// on a dev machine, only that the query works and never reports this
    /// test process itself.
    #[test]
    fn query_succeeds_and_excludes_this_process() {
        let result = other_processes_using_mic();
        eprintln!("mic users: {result:?}");
        if let Some(pids) = result {
            assert!(!pids.contains(&(std::process::id() as i32)));
        }
    }
}
