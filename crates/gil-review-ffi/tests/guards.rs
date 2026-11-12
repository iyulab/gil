//! A value-returning export must not let a panic cross the C boundary: an unwinding panic out of `extern "C"` aborts
//! the host process. The bound for a stratum with no reviewed items is refused as input, with a status and a message.

use std::ptr;

use gil_review_native::*;

#[test]
fn a_bound_for_no_reviewed_items_is_refused_not_fatal() {
    let mut value = f64::NAN;
    let status = unsafe { gil_review_unobserved_variation_bound(0, &mut value) };
    assert_eq!(status, GIL_REVIEW_INVALID_INPUT);
    assert!(value.is_nan(), "the output is left alone on a refusal");
    let len = unsafe { gil_review_last_error(ptr::null_mut(), 0) };
    let mut buffer = vec![0u8; len];
    unsafe { gil_review_last_error(buffer.as_mut_ptr(), buffer.len()) };
    let message = String::from_utf8(buffer).unwrap();
    assert!(message.contains("at least one"), "{message}");
}

#[test]
fn a_bound_with_no_output_is_a_malformed_call() {
    let status = unsafe { gil_review_unobserved_variation_bound(5, ptr::null_mut()) };
    assert_eq!(status, GIL_REVIEW_INVALID_ARGUMENT);
}

#[test]
fn a_bound_for_reviewed_items_is_given() {
    let mut value = f64::NAN;
    let status = unsafe { gil_review_unobserved_variation_bound(5, &mut value) };
    assert_eq!(status, GIL_REVIEW_OK);
    assert!(value > 0.0 && value < 0.25);
}
