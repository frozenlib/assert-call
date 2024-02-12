use std::{
    cell::RefCell,
    marker::PhantomData,
    mem::take,
    sync::{Condvar, Mutex},
};

use crate::CallRecord;

thread_local! {
    static ACTUAL_LOCAL: RefCell<Option<Vec<CallRecord>>> = RefCell::new(None);
}

static ACTUAL_GLOBAL: Mutex<Option<Vec<CallRecord>>> = Mutex::new(None);
static ACTUAL_GLOBAL_CONDVAR: Condvar = Condvar::new();

pub trait Thread {
    fn init() -> Self;
    fn take_actual(&self) -> Vec<CallRecord>;
}

pub struct Local(PhantomData<*mut ()>);

impl Thread for Local {
    fn init() -> Self {
        ACTUAL_LOCAL.with(|actual| {
            let mut actual = actual.borrow_mut();
            if actual.is_some() {
                panic!("CallRecorder::new_local() is already called in this thread");
            }
            *actual = Some(Vec::new());
        });
        Self(PhantomData)
    }
    fn take_actual(&self) -> Vec<CallRecord> {
        ACTUAL_LOCAL.with(|actual| take(actual.borrow_mut().as_mut().unwrap()))
    }
}
impl Drop for Local {
    fn drop(&mut self) {
        ACTUAL_LOCAL.with(|actual| actual.borrow_mut().take());
    }
}

#[non_exhaustive]
pub struct Global {}

impl Thread for Global {
    fn init() -> Self {
        let mut actual = ACTUAL_GLOBAL.lock().unwrap();
        while actual.is_some() {
            actual = ACTUAL_GLOBAL_CONDVAR.wait(actual).unwrap();
        }
        *actual = Some(Vec::new());
        Self {}
    }
    fn take_actual(&self) -> Vec<CallRecord> {
        take(ACTUAL_GLOBAL.lock().unwrap().as_mut().unwrap())
    }
}
impl Drop for Global {
    fn drop(&mut self) {
        ACTUAL_GLOBAL.lock().unwrap().take();
        ACTUAL_GLOBAL_CONDVAR.notify_all();
    }
}

impl CallRecord {
    #[track_caller]
    pub fn record(id: String, file: &'static str, line: u32) {
        ACTUAL_LOCAL.with(|actual| {
            let log = Self { id, file, line };
            if let Some(actual) = &mut *actual.borrow_mut() {
                actual.push(log);
            } else if let Some(seq) = ACTUAL_GLOBAL.lock().unwrap().as_mut() {
                seq.push(log);
            }
        });
    }
}
