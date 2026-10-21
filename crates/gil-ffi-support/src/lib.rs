//! What the C ABIs of the Gil crates share. Each native library links its own copy, so the last error and the last
//! result below are per library and per thread.
//!
//! The convention: every exported function returns a status — [`OK`], or a code whose message the library's
//! `…_last_error` copies out — and runs its body in [`call`], which keeps a panic from crossing the boundary. Text goes
//! in as (pointer, length) UTF-8 ([`Utf8`]); text that comes out (a message, a JSON result) is copied into a buffer the
//! caller owns, with the full length returned so that a call with capacity 0 learns how much to allocate.

use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// The call succeeded.
pub const OK: i32 = 0;
/// The computation refused its input; the message says what was wrong.
pub const INVALID_INPUT: i32 = 1;
/// The call itself was malformed: a null pointer with a length, text that is not UTF-8, or a buffer too small.
pub const INVALID_ARGUMENT: i32 = 2;
/// The computation panicked — a defect in the library, or a call its contract rules out — reported instead of
/// unwinding into the caller.
pub const PANIC: i32 = 3;
/// A buffer the caller gave is too small for the result; the call wrote the sizes it needs where it says, so that the
/// caller can grow its buffers and call again.
pub const BUFFER_TOO_SMALL: i32 = 4;

/// UTF-8 text the caller owns, for the length of one call.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct Utf8 {
    pub ptr: *const u8,
    pub len: usize,
}

thread_local! {
    static LAST_ERROR: RefCell<String> = const { RefCell::new(String::new()) };
    static LAST_RESULT: RefCell<String> = const { RefCell::new(String::new()) };
}

/// A refusal on its way out: the status and the message the last error will give.
#[derive(Debug)]
pub struct Failure(pub i32, pub String);

/// An error the computation returned is a refusal of its input ([`INVALID_INPUT`]), so `?` reports it.
impl<E: std::error::Error> From<E> for Failure {
    fn from(e: E) -> Self {
        Self(INVALID_INPUT, e.to_string())
    }
}

/// A malformed call ([`INVALID_ARGUMENT`]).
pub fn argument(message: impl Into<String>) -> Failure {
    Failure(INVALID_ARGUMENT, message.into())
}

/// Input the computation refuses ([`INVALID_INPUT`]).
pub fn input(message: impl Into<String>) -> Failure {
    Failure(INVALID_INPUT, message.into())
}

/// Runs one call: clears the last error and the last result, keeps a panic from unwinding out, and turns the outcome
/// into a status.
pub fn call(f: impl FnOnce() -> Result<(), Failure>) -> i32 {
    LAST_ERROR.with(|e| e.borrow_mut().clear());
    LAST_RESULT.with(|r| r.borrow_mut().clear());
    let outcome = catch_unwind(AssertUnwindSafe(f)).unwrap_or_else(|panic| {
        let message = panic
            .downcast_ref::<&str>()
            .map(|s| (*s).to_owned())
            .or_else(|| panic.downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "the computation panicked".to_owned());
        Err(Failure(PANIC, message))
    });
    match outcome {
        Ok(()) => OK,
        Err(Failure(status, message)) => {
            LAST_RESULT.with(|r| r.borrow_mut().clear());
            LAST_ERROR.with(|e| *e.borrow_mut() = message);
            status
        }
    }
}

/// Keeps `text` as this thread's last result, for [`copy_last_result`].
pub fn set_result(text: String) {
    LAST_RESULT.with(|r| *r.borrow_mut() = text);
}

/// The length in bytes of this thread's last result.
pub fn last_result_len() -> usize {
    LAST_RESULT.with(|r| r.borrow().len())
}

/// # Safety
/// `buffer` is null with `capacity` 0, or points to `capacity` writable bytes.
unsafe fn copy_out(text: &str, buffer: *mut u8, capacity: usize) -> usize {
    let bytes = text.as_bytes();
    if !buffer.is_null() && capacity >= bytes.len() {
        // SAFETY: the caller guarantees `capacity` writable bytes, and the text fits.
        unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len()) };
    }
    bytes.len()
}

/// Copies the message of the last refused call on this thread into `buffer` (no terminator) and returns its full
/// length; nothing is written when it does not fit. Empty after a call that succeeded.
///
/// # Safety
/// `buffer` is null with `capacity` 0, or points to `capacity` writable bytes.
pub unsafe fn copy_last_error(buffer: *mut u8, capacity: usize) -> usize {
    // SAFETY: forwarded from the caller.
    LAST_ERROR.with(|e| unsafe { copy_out(&e.borrow(), buffer, capacity) })
}

/// Copies the text result of the last call on this thread into `buffer` (no terminator) and returns its full length;
/// nothing is written when it does not fit. Empty after a call that refused, or one with no text result.
///
/// # Safety
/// `buffer` is null with `capacity` 0, or points to `capacity` writable bytes.
pub unsafe fn copy_last_result(buffer: *mut u8, capacity: usize) -> usize {
    // SAFETY: forwarded from the caller.
    LAST_RESULT.with(|r| unsafe { copy_out(&r.borrow(), buffer, capacity) })
}

/// # Safety
/// `ptr` is null with `len` 0, or points to `len` readable values that stay valid for `'a`.
pub unsafe fn slice<'a, T>(ptr: *const T, len: usize, what: &str) -> Result<&'a [T], Failure> {
    if len == 0 {
        return Ok(&[]);
    }
    if ptr.is_null() {
        return Err(argument(format!("{what} is null but its length is {len}")));
    }
    // SAFETY: non-null, and the caller guarantees `len` readable values for 'a.
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}

/// # Safety
/// As [`slice`], writable and aliased by nothing else.
pub unsafe fn slice_mut<'a, T>(
    ptr: *mut T,
    len: usize,
    what: &str,
) -> Result<&'a mut [T], Failure> {
    if len == 0 {
        return Ok(&mut []);
    }
    if ptr.is_null() {
        return Err(argument(format!("{what} is null but its length is {len}")));
    }
    // SAFETY: non-null, and the caller guarantees `len` writable values for 'a, aliased by nothing else.
    Ok(unsafe { std::slice::from_raw_parts_mut(ptr, len) })
}

/// # Safety
/// As [`slice`], for the text's bytes.
pub unsafe fn text<'a>(s: Utf8, what: &str) -> Result<&'a str, Failure> {
    // SAFETY: forwarded from the caller.
    let bytes = unsafe { slice(s.ptr, s.len, what) }?;
    std::str::from_utf8(bytes).map_err(|_| argument(format!("{what} is not UTF-8")))
}

/// # Safety
/// `out` is null or points to one writable `T`.
pub unsafe fn write<T>(out: *mut T, value: T, what: &str) -> Result<(), Failure> {
    if out.is_null() {
        return Err(argument(format!("{what} is null")));
    }
    // SAFETY: non-null, and the caller guarantees one writable T.
    unsafe { out.write(value) };
    Ok(())
}

/// A shared reference to the object behind a handle.
///
/// # Safety
/// `handle` is null or a live handle of this type, used by no other thread during the call.
pub unsafe fn handle<'a, T>(handle: *const T, what: &str) -> Result<&'a T, Failure> {
    // SAFETY: forwarded from the caller.
    unsafe { handle.as_ref() }.ok_or_else(|| argument(format!("{what} is null")))
}

/// An exclusive reference to the object behind a handle.
///
/// # Safety
/// As [`handle`], and nothing else refers to the object during the call.
pub unsafe fn handle_mut<'a, T>(handle: *mut T, what: &str) -> Result<&'a mut T, Failure> {
    // SAFETY: forwarded from the caller.
    unsafe { handle.as_mut() }.ok_or_else(|| argument(format!("{what} is null")))
}

/// Hands `value` out as a new handle through `out`.
///
/// # Safety
/// `out` is null or points to one writable pointer.
pub unsafe fn hand_out<T>(out: *mut *mut T, value: T, what: &str) -> Result<(), Failure> {
    if out.is_null() {
        return Err(argument(format!("{what} is null")));
    }
    // SAFETY: non-null, and the caller guarantees one writable pointer.
    unsafe { out.write(Box::into_raw(Box::new(value))) };
    Ok(())
}

/// Releases a handle [`hand_out`] gave; null does nothing.
///
/// # Safety
/// `handle` is null or a live handle of this type, released once and not used after.
pub unsafe fn release<T>(handle: *mut T) {
    if !handle.is_null() {
        // SAFETY: the caller guarantees a live handle from `Box::into_raw`, released once.
        drop(unsafe { Box::from_raw(handle) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last_error() -> String {
        let len = unsafe { copy_last_error(std::ptr::null_mut(), 0) };
        let mut buffer = vec![0u8; len];
        unsafe { copy_last_error(buffer.as_mut_ptr(), buffer.len()) };
        String::from_utf8(buffer).unwrap()
    }

    fn last_result() -> String {
        let mut buffer = vec![0u8; last_result_len()];
        unsafe { copy_last_result(buffer.as_mut_ptr(), buffer.len()) };
        String::from_utf8(buffer).unwrap()
    }

    #[test]
    fn a_panic_becomes_a_status_and_its_message() {
        assert_eq!(
            call(|| panic!("asked about an attribute that is not decided")),
            PANIC
        );
        assert_eq!(last_error(), "asked about an attribute that is not decided");
        let formatted = 7;
        assert_eq!(call(|| panic!("line {formatted}")), PANIC);
        assert_eq!(last_error(), "line 7");
    }

    #[test]
    fn a_refusal_drops_the_result_and_the_next_call_clears_the_message() {
        assert_eq!(
            call(|| {
                set_result("half".into());
                Err(input("no"))
            }),
            INVALID_INPUT
        );
        assert_eq!(last_error(), "no");
        assert_eq!(last_result(), "");
        assert_eq!(
            call(|| {
                set_result("[1]".into());
                Ok(())
            }),
            OK
        );
        assert_eq!(last_error(), "");
        assert_eq!(last_result(), "[1]");
    }

    #[test]
    fn a_buffer_too_small_is_left_untouched() {
        call(|| Err(argument("twelve bytes")));
        let mut buffer = [0u8; 4];
        assert_eq!(
            unsafe { copy_last_error(buffer.as_mut_ptr(), buffer.len()) },
            12
        );
        assert_eq!(buffer, [0; 4]);
    }

    #[test]
    fn an_error_type_is_a_refusal_of_input() {
        let failure: Failure = "x".parse::<u32>().unwrap_err().into();
        assert_eq!(failure.0, INVALID_INPUT);
    }
}
