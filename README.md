# asesrt-call

[![Crates.io](https://img.shields.io/crates/v/assert-call.svg)](https://crates.io/crates/assert-call)
[![Docs.rs](https://docs.rs/assert-call/badge.svg)](https://docs.rs/assert-call/)
[![Actions Status](https://github.com/frozenlib/assert-call/workflows/CI/badge.svg)](https://github.com/frozenlib/assert-call/actions)

A small utility for testing to verify that the expected parts of the code are called in the expected order.

## Example

```rust :should_panic
use assert_call::{call, CallRecorder};

let mut c = CallRecorder::new();

call!("1");
call!("2");

c.verify(["1", "3"]);
```

The above code panics and outputs the following message because the call to `call!()` is different from what is specified in `verity()`.

```txt
actual calls :
  1
* 2
  (end)

mismatch call
src\lib.rs:10
actual : 2
expect : 3
```

## License

This project is dual licensed under Apache-2.0/MIT. See the two LICENSE-\* files for details.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
