//! macOS's app-wide microphone authorization. CPAL 0.16 erases the native
//! `kAudioUnitErr_Unauthorized` status when building an input stream.

use block2::RcBlock;
use mod_api::MicRefusal;
use objc2::{rc::autoreleasepool, runtime::Bool};
use objc2_av_foundation::{AVAuthorizationStatus as Status, AVCaptureDevice, AVMediaTypeAudio};
use std::sync::mpsc;

pub(super) fn open<T>(start: impl FnOnce() -> Result<T, MicRefusal>) -> Result<T, MicRefusal> {
    // SAFETY: AVFoundation's exported, immutable audio media type. This is one of the two
    // documented valid inputs to the authorization methods below.
    let media_type = unsafe { AVMediaTypeAudio }.ok_or(MicRefusal::DeviceFailed)?;
    with_authorization(
        || {
            // SAFETY: media_type is AVMediaTypeAudio.
            autoreleasepool(|_| unsafe {
                AVCaptureDevice::authorizationStatusForMediaType(media_type)
            })
        },
        || {
            // Only macOS asks for permission. Its callback runs on a dispatch queue, not
            // the client thread or the capture worker waiting here.
            let (tx, rx) = mpsc::sync_channel(1);
            autoreleasepool(|_| {
                let completed = RcBlock::new(move |authorized: Bool| {
                    let _ = tx.send(authorized.as_bool());
                });
                // SAFETY: media_type is AVMediaTypeAudio. The block owns its thread-safe sender
                // and the OS copies the block if it calls it after this method returns.
                unsafe {
                    AVCaptureDevice::requestAccessForMediaType_completionHandler(
                        media_type, &completed,
                    );
                }
            });
            rx.recv().map_err(|_| MicRefusal::DeviceFailed)
        },
        start,
    )
}

fn with_authorization<T>(
    status: impl Fn() -> Status,
    request: impl FnOnce() -> Result<bool, MicRefusal>,
    start: impl FnOnce() -> Result<T, MicRefusal>,
) -> Result<T, MicRefusal> {
    match status() {
        Status::Authorized => {}
        Status::NotDetermined => {
            if !request()? {
                return Err(MicRefusal::OsDenied);
            }
        }
        Status::Denied | Status::Restricted => return Err(MicRefusal::OsDenied),
        _ => return Err(MicRefusal::DeviceFailed),
    }
    let result = start();
    // Permission may change during startup. In particular a CPAL DeviceNotAvailable
    // is ambiguous by itself: return OsDenied only if the OS reports it.
    match status() {
        Status::Denied | Status::Restricted => Err(MicRefusal::OsDenied),
        _ => result,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denied_or_restricted_authorization_does_not_start_capture() {
        for status in [Status::Denied, Status::Restricted] {
            assert_eq!(
                with_authorization(
                    || status,
                    || panic!("must not request again"),
                    || -> Result<(), _> { panic!("must not start capture") },
                ),
                Err(MicRefusal::OsDenied)
            );
        }
    }

    #[test]
    fn a_denied_first_os_request_is_not_a_device_error() {
        assert_eq!(
            with_authorization(
                || Status::NotDetermined,
                || Ok(false),
                || -> Result<(), _> { panic!("must not start capture") },
            ),
            Err(MicRefusal::OsDenied)
        );
    }

    #[test]
    fn denial_during_startup_is_not_reported_as_device_loss() {
        let status = std::cell::Cell::new(Status::Authorized);
        assert_eq!(
            with_authorization(
                || status.get(),
                || panic!("must not request again"),
                || {
                    status.set(Status::Denied);
                    Err::<(), _>(MicRefusal::NoDevice)
                },
            ),
            Err(MicRefusal::OsDenied)
        );
    }

    #[test]
    fn a_missing_device_with_os_permission_remains_no_device() {
        assert_eq!(
            with_authorization(
                || Status::Authorized,
                || panic!("must not request again"),
                || Err::<(), _>(MicRefusal::NoDevice),
            ),
            Err(MicRefusal::NoDevice)
        );
    }
}
