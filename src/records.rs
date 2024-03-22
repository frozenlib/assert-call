use std::{
    backtrace::{Backtrace, BacktraceStatus},
    cell::RefCell,
    cmp::min,
    fmt::{self, Formatter},
    marker::PhantomData,
    mem::take,
    sync::{Condvar, Mutex},
    thread,
};

use yansi::{Condition, Paint};

use crate::Record;

thread_local! {
    static ACTUAL_LOCAL: RefCell<Option<Vec<Record>>> = const { RefCell::new(None) };
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

#[derive(Debug)]
pub struct Records(pub(crate) Vec<Record>);

impl Records {
    pub(crate) fn empty() -> Self {
        Self(Vec::new())
    }

    #[track_caller]
    pub fn push(id: String, file: &'static str, line: u32) {
        let record = Record {
            id,
            file,
            line,
            backtrace: Backtrace::capture(),
            thread_id: thread::current().id(),
        };
        let used = ACTUAL_LOCAL.with(|actual| {
            if let Some(actual) = &mut *actual.borrow_mut() {
                actual.push(record);
                true
            } else if let Some(seq) = ACTUAL_GLOBAL.lock().unwrap().as_mut() {
                seq.push(record);
                true
            } else {
                false
            }
        });
        if !used {
            panic!("`CallRecorder` is not initialized.");
        }
    }

    fn id(&self, index: usize) -> &str {
        if let Some(a) = self.0.get(index) {
            &a.id
        } else {
            "(end)"
        }
    }

    pub(crate) fn fmt_summary(
        &self,
        f: &mut Formatter,
        mismatch_index: usize,
        around: usize,
        color: bool,
    ) -> fmt::Result {
        let mut start = 0;
        let end = self.0.len();
        if mismatch_index > around {
            start = mismatch_index - around;
        }
        let end = min(mismatch_index + around + 1, end);
        if start > 0 {
            writeln!(f, "  ...(previous {start} calls omitted)")?;
        }
        for index in start..end {
            self.fmt_item_summary(f, mismatch_index == index, self.id(index), color)?;
        }
        if end == self.0.len() {
            self.fmt_item_summary(f, mismatch_index == self.0.len(), "(end)", color)?;
        } else {
            writeln!(f, "  ...(following {} calls omitted)", self.0.len() - end)?;
        }
        Ok(())
    }
    fn fmt_item_summary(
        &self,
        f: &mut Formatter,
        is_mismatch: bool,
        id: &str,
        color: bool,
    ) -> fmt::Result {
        let head = if is_mismatch { "*" } else { " " };
        let cond = if is_mismatch && color {
            Condition::ALWAYS
        } else {
            Condition::NEVER
        };
        writeln!(f, "{}", format_args!("{head} {id}").red().whenever(cond))
    }
    pub(crate) fn fmt_backtrace(
        &self,
        f: &mut Formatter,
        mismatch_index: usize,
        around: usize,
    ) -> fmt::Result {
        let mut start = 0;
        let end = self.0.len();
        if mismatch_index > around {
            start = mismatch_index - around;
        }
        let end = min(mismatch_index + 1, end);
        if start > 0 {
            writeln!(f, "# ...(previous {start} calls omitted)")?;
        }
        for index in start..end {
            let r = &self.0[index];
            writeln!(f, "# {}", r.id)?;
            writeln!(f, "{}:{}", r.file, r.line)?;
            writeln!(f, "thread: {:?}", r.thread_id)?;
            writeln!(f, "{}", r.backtrace)?;
        }

        if end == self.0.len() {
            writeln!(f, "# (end)")?;
        } else {
            writeln!(f, "  ...(following {} calls omitted)", self.0.len() - end)?;
        }
        Ok(())
    }

    pub(crate) fn has_bakctrace(&self) -> bool {
        self.0
            .iter()
            .any(|r| r.backtrace.status() == BacktraceStatus::Captured)
    }
}
