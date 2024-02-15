use assert_call::call;

// Use a file containing only a single test,
// as multiple tests in a file can cause multiple tests to run simultaneously in the same process and initialize `CallRecorder` with other tests
#[should_panic]
#[test]
fn no_call_recorder() {
    call!("0");
}
