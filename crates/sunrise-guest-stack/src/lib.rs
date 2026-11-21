//! Legacy stack reservation and inspection used by the region writer stack test.

#[path = "guest_stack_constants.rs"]
mod constants;
pub use constants::{GUEST_STACK_BYTES, HOST_HEADROOM};

/// Stack reservation used by game threads that call mods.
pub const fn reservation() -> usize {
    GUEST_STACK_BYTES + HOST_HEADROOM
}

/// Spawn guarantees the reservation even on platforms without a stack-size query.
pub fn builder() -> std::thread::Builder {
    std::thread::Builder::new().stack_size(reservation())
}

/// Starts a thread that may enter a guest: guest-sized and named `name`, and in debug builds it
/// asserts at its start that the reservation took, so a test that starts it fails loudly if not.
pub fn spawn<T: Send + 'static>(
    name: String,
    body: impl FnOnce() -> T + Send + 'static,
) -> std::io::Result<std::thread::JoinHandle<T>> {
    builder().name(name).spawn(move || {
        debug_assert_eq!(
            check_current(),
            Ok(()),
            "a guest-capable thread must start on the guest stack"
        );
        body()
    })
}

/// Check that the calling thread has the expected stack reservation.
/// Cache the platform query once per thread.
pub fn check_current() -> Result<(), &'static str> {
    thread_local! {
        static CHECK: std::cell::OnceCell<Result<(), &'static str>> = const { std::cell::OnceCell::new() };
    }
    CHECK.with(|check| {
        *check.get_or_init(|| {
            inspect_current().map_err(|error| match error {
                #[cfg(not(any(target_vendor = "apple", windows)))]
                StackError::Uninspectable(reason) => reason,
                StackError::Small => "GuestStackUnavailable: calling thread needs 136 MiB of stack",
            })
        })
    })
}

enum StackError {
    /// Linux and other non-Apple, non-Windows targets: the stack could not be read.
    #[cfg(not(any(target_vendor = "apple", windows)))]
    Uninspectable(&'static str),
    /// The calling thread's stack is smaller than the expected reservation.
    Small,
}

fn inspect_current() -> Result<(), StackError> {
    let size = current_stack_size()?;
    if !fits(size, THREAD_STRUCTURE_ALLOWANCE) {
        return Err(StackError::Small);
    }
    Ok(())
}

/// Bionic's `pthread_getattr_np` reports a thread's stack without the thread's own structure,
/// which sits at its top: a few KiB short of what the thread was asked for, so 16 KiB is
/// accepted there. Every other platform reports the full size.
const THREAD_STRUCTURE_ALLOWANCE: usize = if cfg!(target_os = "android") {
    16 * 1024
} else {
    0
};

/// Whether a stack of `size` bytes holds the guest's reservation, short by at most `allowance`.
const fn fits(size: usize, allowance: usize) -> bool {
    size.saturating_add(allowance) >= GUEST_STACK_BYTES + HOST_HEADROOM
}

#[cfg(any(target_os = "linux", target_os = "android"))]
fn current_stack_size() -> Result<usize, StackError> {
    #[cfg(target_os = "linux")]
    {
        // Linux only: Android's main thread goes through pthread_getattr_np like any other.
        // SAFETY: getpid and gettid take no arguments and always succeed.
        let main = unsafe { libc::getpid() as libc::c_long == libc::syscall(libc::SYS_gettid) };
        if main {
            // The main thread's stack grows on demand up to RLIMIT_STACK.
            // glibc's pthread_getattr_np reports that limit, but musl's reports only
            // the pages mapped so far, so read the limit itself.
            let mut limit = libc::rlimit {
                rlim_cur: 0,
                rlim_max: 0,
            };
            // SAFETY: limit is writable and getrlimit initializes it on success.
            if unsafe { libc::getrlimit(libc::RLIMIT_STACK, &mut limit) } != 0 {
                return Err(StackError::Uninspectable(
                    "GuestStackUnavailable: cannot read the main thread's stack limit",
                ));
            }
            return Ok(usize::try_from(limit.rlim_cur).unwrap_or(usize::MAX));
        }
    }
    let mut attr = std::mem::MaybeUninit::<libc::pthread_attr_t>::uninit();
    // SAFETY: pthread_self is live and attr is writable. Destroy only on success.
    unsafe {
        if libc::pthread_getattr_np(libc::pthread_self(), attr.as_mut_ptr()) != 0 {
            return Err(StackError::Uninspectable(
                "GuestStackUnavailable: cannot inspect the calling thread stack",
            ));
        }
        let mut attr = attr.assume_init();
        let mut size = 0;
        let result = libc::pthread_attr_getstacksize(&attr, &mut size);
        libc::pthread_attr_destroy(&mut attr);
        if result != 0 {
            return Err(StackError::Uninspectable(
                "GuestStackUnavailable: cannot inspect the calling thread stack",
            ));
        }
        Ok(size)
    }
}

#[cfg(target_vendor = "apple")]
fn current_stack_size() -> Result<usize, StackError> {
    // SAFETY: pthread_self names the live calling thread.
    Ok(unsafe { libc::pthread_get_stacksize_np(libc::pthread_self()) })
}

#[cfg(windows)]
fn current_stack_size() -> Result<usize, StackError> {
    let mut low = 0;
    let mut high = 0;
    // SAFETY: both pointers refer to writable stack bounds.
    unsafe { get_current_thread_stack_limits(&mut low, &mut high) };
    Ok(high - low)
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "android",
    target_vendor = "apple",
    windows
)))]
fn current_stack_size() -> Result<usize, StackError> {
    Err(StackError::Uninspectable(
        "GuestStackUnavailable: stack inspection is unsupported on this platform",
    ))
}

#[cfg(windows)]
#[link(name = "kernel32")]
unsafe extern "system" {
    #[link_name = "GetCurrentThreadStackLimits"]
    fn get_current_thread_stack_limits(low: *mut usize, high: *mut usize);
}

#[cfg(test)]
mod tests {
    use super::{GUEST_STACK_BYTES, HOST_HEADROOM, fits};

    #[test]
    fn the_thread_structure_allowance_covers_a_few_kib_and_no_more() {
        let required = 136 * 1024 * 1024;
        assert!(fits(required, 0));
        assert!(!fits(required - 4096, 0));
        assert!(fits(required - 4096, 16 * 1024));
        assert!(!fits(required - 1024 * 1024, 16 * 1024));
    }

    #[test]
    fn the_largest_stack_fits_with_an_allowance() {
        assert!(fits(usize::MAX, 16 * 1024));
        assert!(fits(usize::MAX, 0));
    }

    #[test]
    fn the_allowance_boundary_is_exact() {
        let required = GUEST_STACK_BYTES + HOST_HEADROOM;
        assert!(fits(required - 16 * 1024, 16 * 1024));
        assert!(!fits(required - 16 * 1024 - 1, 16 * 1024));
        assert!(!fits(0, 16 * 1024));
    }

    #[test]
    fn small_worker_refuses_guest_entry() {
        std::thread::Builder::new()
            .stack_size(1024 * 1024)
            .spawn(|| {
                assert!(
                    super::check_current()
                        .unwrap_err()
                        .contains("GuestStackUnavailable")
                )
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn guest_worker_reserves_the_required_stack() {
        super::builder()
            .spawn(|| super::check_current().unwrap())
            .unwrap()
            .join()
            .unwrap();
    }
}
