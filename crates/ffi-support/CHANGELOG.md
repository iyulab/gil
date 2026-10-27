# Changelog — `iyulab-ffi-support`

0.1.0 is the first release. This crate has its own version line, apart from the Gil crates. Versions follow `0.x`: a
minor release may change the public API, and each such change is listed under **Breaking** with what to do.

## 0.1.0

### Added

- The crate, published for the first time: the statuses `OK` · `INVALID_INPUT` · `INVALID_ARGUMENT` · `PANIC` ·
  `BUFFER_TOO_SMALL` · `BROKEN`, the thread-local last error and last result with their copy-out functions, the panic
  guard `call`, `Utf8` and the checked readers and writers, and the handle functions — the convention the C ABIs of the
  Gil crates already speak, unchanged.
