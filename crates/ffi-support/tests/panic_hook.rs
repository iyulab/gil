//! The panic hook `call` installs is quiet for a panic inside a call and leaves every other panic to the hook that was
//! set before. The process-wide hook is global, so these tests take turns and set their own hook before the first call.

use iyulab_ffi_support::{PANIC, call};
use std::panic::{self, catch_unwind};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, Once};

static NOTED: AtomicUsize = AtomicUsize::new(0);
static TURN: Mutex<()> = Mutex::new(());

/// Takes the turn and makes sure the hook that counts panics is the one `call` will find.
fn take_turn() -> MutexGuard<'static, ()> {
    let turn = TURN.lock().unwrap_or_else(|e| e.into_inner());
    static COUNTING: Once = Once::new();
    COUNTING.call_once(|| {
        panic::set_hook(Box::new(|_| {
            NOTED.fetch_add(1, Ordering::SeqCst);
        }));
    });
    NOTED.store(0, Ordering::SeqCst);
    turn
}

#[test]
fn a_panic_inside_a_call_is_not_noted_by_the_hook_that_was_set_before() {
    let _turn = take_turn();
    assert_eq!(call(|| panic!("inside a call")), PANIC);
    assert_eq!(NOTED.load(Ordering::SeqCst), 0);
}

#[test]
fn a_panic_outside_a_call_still_reaches_the_hook_that_was_set_before() {
    let _turn = take_turn();
    assert_eq!(call(|| Ok(())), 0);
    assert!(catch_unwind(|| -> u8 { panic!("outside a call") }).is_err());
    assert_eq!(NOTED.load(Ordering::SeqCst), 1);
    // A call that has ended leaves the thread outside a call again, even when its body panicked.
    assert_eq!(call(|| panic!("inside a call")), PANIC);
    assert!(catch_unwind(|| -> u8 { panic!("outside again") }).is_err());
    assert_eq!(NOTED.load(Ordering::SeqCst), 2);
}
