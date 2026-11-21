//! AVFoundation's application-wide microphone authorization.

use block2::RcBlock;
use objc2::{rc::autoreleasepool, runtime::Bool};
use objc2_av_foundation::{AVCaptureDevice, AVMediaTypeAudio};
use std::sync::mpsc;

pub use objc2_av_foundation::AVAuthorizationStatus as MicrophoneAuthorization;

/// Reads the OS microphone authorization without asking for permission.
pub fn microphone_authorization() -> Option<MicrophoneAuthorization> {
    // SAFETY: This immutable framework export is a documented input to the authorization API.
    let audio = unsafe { AVMediaTypeAudio }?;
    // SAFETY: AVMediaTypeAudio is one of the two media types accepted by the method.
    Some(autoreleasepool(|_| unsafe {
        AVCaptureDevice::authorizationStatusForMediaType(audio)
    }))
}

/// Waits on the capture worker for the OS's microphone permission decision.
pub fn request_microphone_access() -> Option<bool> {
    // SAFETY: This immutable framework export is a documented input to the authorization API.
    let audio = unsafe { AVMediaTypeAudio }?;
    let (tx, rx) = mpsc::sync_channel(1);
    autoreleasepool(|_| {
        let completed = RcBlock::new(move |authorized: Bool| {
            let _ = tx.send(authorized.as_bool());
        });
        // SAFETY: The media type is AVMediaTypeAudio. AVFoundation copies the block for
        // asynchronous use; it owns the thread-safe sender and borrows no Rust state.
        unsafe {
            AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &completed);
        }
    });
    // AVFoundation calls the block on a dispatch queue, not on this waiting thread.
    rx.recv().ok()
}
