use pretty_assertions::assert_str_eq;

use crate::{call, thread::Thread, Call, CallRecorder};

#[test]
fn err() {
    let c = CallRecorder::new_local();
    call!("0");
    assert_err(
        c,
        ["1"],
        r#"
actual calls :
* 0
  (end)

(message)
tests\test.rs:10
actual : 0
expect : 1"#,
    );
}

#[test]
fn err_many_expect() {
    let c = CallRecorder::new_local();
    call!("0");
    assert_err(
        c,
        Call::any(["1", "2", "1", "2"]),
        r#"
actual calls :
* 0
  (end)

(message)
tests\test.rs:10
actual : 0
expect : 1, 2"#,
    );
}

#[test]
fn many_calls() {
    let c = CallRecorder::new_local();
    for i in 0..20 {
        if i == 10 {
            call!("None");
        } else {
            call!("{i}");
        }
    }
    assert_err(
        c,
        Call::seq(0..20),
        r#"
actual calls :
  ...(previous 5 calls omitted)
  5
  6
  7
  8
  9
* None
  11
  12
  13
  14
  15
  ...(following 4 calls omitted)

(message)
tests\test.rs:10
actual : None
expect : 10"#,
    );
}

fn assert_err(mut c: CallRecorder<impl Thread>, expect: impl Into<Call>, expect_display: &str) {
    match c.result_with_msg(expect, "(message)") {
        Ok(_) => panic!("no error."),
        Err(mut e) => {
            e.set_dummy_file_line();
            let actual = e.to_string();
            let actual = actual.trim();
            let expect = expect_display.trim();
            assert_str_eq!(actual, expect, "\n----\n{actual}\n----");
        }
    }
}
