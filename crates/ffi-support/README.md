# iyulab-ffi-support

The plumbing of a C ABI over Rust, as one convention that several native libraries can share. Each library links its
own copy, so the state below is per library and per thread.

- **Statuses.** Every exported function returns an `i32`: `OK` (0), `INVALID_INPUT` (1 — the computation refused its
  input), `INVALID_ARGUMENT` (2 — a null pointer with a length, text that is not UTF-8, a buffer too small),
  `PANIC` (3 — reported instead of unwinding into the caller), `BUFFER_TOO_SMALL` (4 — the call wrote the sizes it
  needs) and `BROKEN` (5 — a handle whose earlier change panicked partway; free it and build it again).
- **The last error and the last result.** A refusal leaves its message in a thread-local last error; a call that
  produces text (a JSON result) leaves it in a thread-local last result. `copy_last_error` and `copy_last_result`
  copy into a buffer the caller owns and return the full length, so a call with capacity 0 learns how much to
  allocate.
- **The panic guard.** `call` runs a body that returns `Result<(), Failure>`, turns a failure into its status and
  message, and keeps a panic from crossing the boundary. The panic's note on stderr is silenced only for a panic
  inside a `call` — its message reaches the host as the last error. A panic anywhere else in the process, such as a
  failing assertion in a Rust test, is reported by the hook that was set before the first `call`.
- **Reading caller memory.** `Utf8` is (pointer, length) UTF-8 the caller owns for one call; `text`, `slice`,
  `slice_mut` and `write` check pointers and lengths before they are read or written.
- **Handles.** `handle` (`*const` — a read, which may run on several threads at once), `handle_mut` (`*mut` — a
  change, which needs the handle to itself), `hand_out` and `release`.

```rust
use iyulab_ffi_support::{Utf8, call, input, text};

#[unsafe(no_mangle)]
pub unsafe extern "C" fn my_parse(source: Utf8) -> i32 {
    call(|| {
        let s = unsafe { text(source, "source") }?;
        if s.is_empty() {
            return Err(input("source is empty"));
        }
        Ok(())
    })
}
```

A library usually re-exports the statuses under its own prefix (`MYLIB_OK = iyulab_ffi_support::OK`) so that its
header speaks one vocabulary.

## License

Apache-2.0
