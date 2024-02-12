use std::{
    sync::atomic::{AtomicUsize, Ordering},
    thread::{scope, sleep, spawn},
    time::Duration,
};

use pretty_assertions::assert_eq;

use assert_call::{call, Call, CallRecorder};

#[test]
fn new() {
    let mut c = CallRecorder::new();
    call!("1");

    c.verify("1");
}

#[test]
fn new_thread() {
    let mut c = CallRecorder::new();
    spawn(|| call!("1")).join().unwrap();

    c.verify("1");
}

#[test]
fn new_parallel() {
    let value = AtomicUsize::new(0);
    let value_max = AtomicUsize::new(0);
    scope(|s| {
        for _ in 0..10 {
            s.spawn(|| {
                let _ac = CallRecorder::new();
                value.fetch_add(1, Ordering::SeqCst);
                sleep(Duration::from_millis(100));
                value_max.fetch_max(value.load(Ordering::SeqCst), Ordering::SeqCst);
                value.fetch_sub(1, Ordering::SeqCst);
            });
        }
    });
    assert_eq!(value_max.load(Ordering::SeqCst), 1);
}

#[test]
fn new_local() {
    let mut c = CallRecorder::new_local();
    call!("1");

    c.verify("1");
}

#[should_panic]
#[test]
fn new_local_nested() {
    let _cr = CallRecorder::new_local();
    let _cr2 = CallRecorder::new_local();
}

#[should_panic]
#[test]
fn extra_call() {
    let mut c = CallRecorder::new_local();
    call!("1");
    c.verify(());
}

#[should_panic]
#[test]
fn not_call() {
    let mut c = CallRecorder::new_local();
    c.verify("1");
}

#[should_panic]
#[test]
fn not_expect() {
    let mut c = CallRecorder::new_local();
    call!("1");
    c.verify(());
}

#[test]
fn verify_2() {
    let mut c = CallRecorder::new();
    call!("1");
    call!("2");
    c.verify(["1", "2"]);

    call!("3");
    call!("4");
    c.verify(["3", "4"]);
}

#[test]
fn verify_2_local() {
    let mut c = CallRecorder::new_local();
    call!("1");
    call!("2");
    c.verify(["1", "2"]);

    call!("3");
    call!("4");
    c.verify(["3", "4"]);
}
#[test]
fn id() {
    let mut c = CallRecorder::new_local();
    call!("1");
    c.verify(Call::id("1"));
}

#[test]
fn seq_array() {
    let mut c = CallRecorder::new_local();
    call!("1");
    call!("2");
    call!("3");

    c.verify(["1", "2", "3"]);
}

#[test]
fn seq_vec() {
    let mut c = CallRecorder::new_local();
    call!("1");
    call!("2");
    call!("3");

    c.verify(vec!["1", "2", "3"]);
}

#[should_panic]
#[test]
fn seq_fail_not_call() {
    let mut c = CallRecorder::new_local();
    call!("1");
    c.verify(["1", "2"]);
}

#[should_panic]
#[test]
fn seq_fail_wrong_order() {
    let mut c = CallRecorder::new_local();
    call!("2");
    call!("1");
    c.verify(["1", "2"]);
}

#[test]
fn any() {
    let mut c = CallRecorder::new_local();
    let expect = Call::any(["1", "2", "3"]);
    call!("1");
    c.verify(&expect);
    call!("3");
    c.verify(&expect);
    call!("2");
    c.verify(&expect);
}

#[should_panic]
#[test]
fn any_fail_not_match() {
    let mut c = CallRecorder::new_local();
    let expect = Call::any(["1", "2", "3"]);
    call!("4");
    c.verify(&expect);
}

#[should_panic]
#[test]
fn any_fail_not_call() {
    let mut c = CallRecorder::new_local();
    let expect = Call::any(["1", "2", "3"]);
    c.verify(&expect);
}

#[should_panic]
#[test]
fn any_fail_extra_call() {
    let mut c = CallRecorder::new_local();
    let expect = Call::any(["1", "2", "3"]);
    call!("1");
    call!("2");
    c.verify(&expect);
}

#[test]
fn any_seq() {
    let mut c = CallRecorder::new_local();
    let expect = Call::any([["1", "2"], ["a", "b"]]);
    call!("a");
    call!("b");
    c.verify(&expect);
    call!("1");
    call!("2");
    c.verify(&expect);
}

#[test]
fn par() {
    let mut c = CallRecorder::new_local();
    let expect = Call::par(["1", "2", "3"]);
    call!("1");
    call!("2");
    call!("3");
    c.verify(&expect);

    call!("3");
    call!("1");
    call!("2");
    c.verify(&expect);
}

#[test]
fn par_seq() {
    let mut c = CallRecorder::new_local();
    let expect = Call::par([["1", "2"], ["a", "b"]]);
    call!("1");
    call!("a");
    call!("b");
    call!("2");
    c.verify(&expect);
}

#[test]
fn call_format() {
    let mut c = CallRecorder::new_local();
    let x = 10;
    call!("a_{x}");
    call!("b_{}", x);
    c.verify(["a_10", "b_10"]);
}
