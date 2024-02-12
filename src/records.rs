use std::{
    cell::RefCell,
    marker::PhantomData,
    mem::take,
    sync::{Condvar, Mutex},
};

use crate::Record;

thread_local! {
    static ACTUAL_LOCAL: RefCell<Option<Vec<Record>>> = RefCell::new(None);
}

static ACTUAL_GLOBAL: Mutex<Option<Vec<Record>>> = Mutex::new(None);
static ACTUAL_GLOBAL_CONDVAR: Condvar = Condvar::new();

pub trait Thread {
    fn init() -> Self;
    fn take_actual(&self) -> Records;
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
    fn take_actual(&self) -> Records {
        Records(ACTUAL_LOCAL.with(|actual| take(actual.borrow_mut().as_mut().unwrap())))
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
    fn take_actual(&self) -> Records {
        Records(take(ACTUAL_GLOBAL.lock().unwrap().as_mut().unwrap()))
    }
}
impl Drop for Global {
    fn drop(&mut self) {
        ACTUAL_GLOBAL.lock().unwrap().take();
        ACTUAL_GLOBAL_CONDVAR.notify_all();
    }
}

pub struct Records(pub(crate) Vec<Record>);

impl Records {
    #[track_caller]
    pub fn push(id: String, file: &'static str, line: u32) {
        ACTUAL_LOCAL.with(|actual| {
            let r = Record { id, file, line };
            if let Some(actual) = &mut *actual.borrow_mut() {
                actual.push(r);
            } else if let Some(seq) = ACTUAL_GLOBAL.lock().unwrap().as_mut() {
                seq.push(r);
            }
        });
    }
}
