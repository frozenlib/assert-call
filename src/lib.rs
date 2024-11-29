//! A tool for testing that ensures code parts are called as expected.
//!
//! By creating an instance of [`CallRecorder`],
//! it starts recording the calls to [`call`], and then [`CallRecorder::verify`] verifies that the calls to [`call`] are as expected.
//!
//! The pattern of expected calls specified in [`CallRecorder::verify`] uses [`Call`].
//!
//! ## Examples
//!
//! ```should_panic
//! use assert_call::{call, CallRecorder};
//!
//! let mut c = CallRecorder::new();
//!
//! call!("1");
//! call!("2");
//!
//! c.verify(["1", "3"]);
//! ```
//!
//! The above code panics and outputs the following message
//! because the call to [`call`] macro is different from what is specified in [`CallRecorder::verify`].
//!
//! ```txt
//! actual calls :
//!   1
//! * 2
//!   (end)
//!
//! mismatch call
//! src\lib.rs:10
//! actual : 2
//! expect : 3
//! ```
//!
//! # Backtrace support
//!
//! If backtrace capture is enabled at [`Backtrace::capture`],
//! [`CallRecorder::verify`] outputs detailed information including the backtrace for each [`call!`] call.
//!
use std::{
    backtrace::{Backtrace, BacktraceStatus},
    collections::VecDeque,
    error::Error,
    fmt::Display,
    thread::{self, ThreadId},
};

use records::{Global, Local, Records, Thread};
use yansi::Condition;

pub mod records;

#[cfg(test)]
mod tests;

/// Record the call.
///
/// The argument is the call ID with the same format as [`std::format`].
///
/// # Panics
///
/// Panics if [`CallRecorder`] is not initialized.
///
/// If `call!()` is allowed to be called while `CallRecorder` is not initialized,
/// the test result will be wrong
/// if a test that initializes `CallRecorder` and a test in which `CallRecorder` is not initialized are performed at the same time,
/// so calling `call!()` without initializing `CallRecorder` is not allowed.
///
/// # Examples
///
/// ```
/// use assert_call::call;
/// let c = assert_call::CallRecorder::new_local();
///
/// call!("1");
/// call!("{}-{}", 1, 2);
/// ```
#[macro_export]
macro_rules! call {
    ($($id:tt)*) => {
        $crate::records::Records::push(::std::format!($($id)*), ::std::file!(), ::std::line!());
    };
}

/// Records and verifies calls to [`call`].
pub struct CallRecorder<T: Thread = Global> {
    thread: T,
}
impl CallRecorder {
    /// Start recording [`call`] macro calls in all threads.
    ///
    /// If there are other instances of `CallRecorder` created by this function,
    /// wait until the other instances are dropped.
    pub fn new() -> Self {
        Self::new_raw()
    }
}
impl CallRecorder<Local> {
    /// Start recording [`call`] macro calls in current thread.
    ///
    /// # Panics
    ///
    /// Panics if an instance of `CallRecorder` created by `new_local` already exists in this thread.
    pub fn new_local() -> Self {
        Self::new_raw()
    }
}
impl<T: Thread> CallRecorder<T> {
    fn new_raw() -> Self {
        Self { thread: T::init() }
    }

    /// Panic if [`call`] call does not match the expected pattern.
    ///
    /// Calling this method clears the recorded [`call`] calls.
    #[track_caller]
    pub fn verify(&mut self, expect: impl ToCall) {
        self.verify_with_msg(expect, "mismatch call");
    }

    /// Panic with specified message if [`call`] call does not match the expected pattern.
    ///
    /// Calling this method clears the recorded [`call`] calls.
    #[track_caller]
    pub fn verify_with_msg(&mut self, expect: impl ToCall, msg: &str) {
        match self.result_with_msg(expect, msg) {
            Ok(_) => {}
            Err(e) => {
                panic!("{:#}", e.display(true, Condition::tty_and_color()));
            }
        }
    }

    /// Return `Err` with specified message if [`call`] call does not match the expected pattern.
    ///
    /// Calling this method clears the recorded [`call`] calls.
    fn result_with_msg(&mut self, expect: impl ToCall, msg: &str) -> Result<(), CallMismatchError> {
        let expect: Call = expect.to_call();
        let actual = self.thread.take_actual();
        expect.verify(actual, msg)
    }
}
impl<T: Thread> Default for CallRecorder<T> {
    fn default() -> Self {
        Self::new_raw()
    }
}
impl<T: Thread> Drop for CallRecorder<T> {
    fn drop(&mut self) {}
}

/// Pattern of expected [`call`] calls.
///
/// To create a value of this type, call a method of this type or use [`ToCall`].
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Call {
    Id(String),
    Seq(VecDeque<Call>),
    Par(Vec<Call>),
    Any(Vec<Call>),
}

impl Call {
    /// Create `Call` to represent a single [`call`] call.
    ///
    /// # Examples
    ///
    /// ```
    /// use assert_call::{call, Call, CallRecorder};
    ///
    /// let mut c = CallRecorder::new();
    /// call!("1");
    /// c.verify(Call::id("1"));
    /// ```
    pub fn id(id: impl Display) -> Self {
        Self::Id(id.to_string())
    }

    /// Create `Call` to represent no [`call`] call.
    ///
    /// # Examples
    ///
    /// ```
    /// use assert_call::{Call, CallRecorder};
    ///
    /// let mut c = CallRecorder::new();
    /// c.verify(Call::empty());
    /// ```
    pub fn empty() -> Self {
        Self::Seq(VecDeque::new())
    }

    /// Create `Call` to represent all specified `Call`s will be called in sequence.
    ///
    /// # Examples
    ///
    /// ```
    /// use assert_call::{call, Call, CallRecorder};
    ///
    /// let mut c = CallRecorder::new();
    /// call!("1");
    /// call!("2");
    /// c.verify(Call::seq(["1", "2"]));
    /// ```
    pub fn seq(p: impl IntoIterator<Item = impl ToCall>) -> Self {
        Self::Seq(p.into_iter().map(|x| x.to_call()).collect())
    }

    /// Create `Call` to represent all specified `Call`s will be called in parallel.
    ///
    /// # Examples
    ///
    /// ```
    /// use assert_call::{call, Call, CallRecorder};
    ///
    /// let mut c = CallRecorder::new();
    /// call!("a-1");
    /// call!("b-1");
    /// call!("b-2");
    /// call!("a-2");
    /// c.verify(Call::par([["a-1", "a-2"], ["b-1", "b-2"]]));
    /// ```
    pub fn par(p: impl IntoIterator<Item = impl ToCall>) -> Self {
        Self::Par(p.into_iter().map(|x| x.to_call()).collect())
    }

    /// Create `Call` to represent one of the specified `Call`s will be called.
    ///
    /// # Examples
    ///
    /// ```
    /// use assert_call::{call, Call, CallRecorder};
    ///
    /// let mut c = CallRecorder::new();
    /// call!("1");
    /// c.verify(Call::any(["1", "2"]));
    /// call!("4");
    /// c.verify(Call::any(["3", "4"]));
    /// ```
    pub fn any(p: impl IntoIterator<Item = impl ToCall>) -> Self {
        Self::Any(p.into_iter().map(|x| x.to_call()).collect())
    }

    fn verify(mut self, actual: Records, msg: &str) -> Result<(), CallMismatchError> {
        match self.verify_nexts(&actual.0) {
            Ok(_) => Ok(()),
            Err(mut e) => {
                e.actual = actual;
                e.expect.sort();
                e.expect.dedup();
                e.msg = msg.to_string();
                Err(e)
            }
        }
    }
    fn verify_nexts(&mut self, actual: &[Record]) -> Result<(), CallMismatchError> {
        for index in 0..=actual.len() {
            self.verify_next(index, actual.get(index))?;
        }
        Ok(())
    }
    fn verify_next(&mut self, index: usize, a: Option<&Record>) -> Result<(), CallMismatchError> {
        if let Err(e) = self.next(a) {
            if a.is_none() && e.is_empty() {
                return Ok(());
            }
            Err(CallMismatchError::new(e, index))
        } else {
            Ok(())
        }
    }

    fn next(&mut self, p: Option<&Record>) -> Result<(), Vec<String>> {
        match self {
            Call::Id(id) => {
                if Some(id.as_str()) == p.as_ref().map(|x| x.id.as_str()) {
                    *self = Call::Seq(VecDeque::new());
                    Ok(())
                } else {
                    Err(vec![id.to_string()])
                }
            }
            Call::Seq(list) => {
                while !list.is_empty() {
                    match list[0].next(p) {
                        Err(e) if e.is_empty() => list.pop_front(),
                        ret => return ret,
                    };
                }
                Err(Vec::new())
            }
            Call::Par(s) => {
                let mut es = Vec::new();
                for i in s.iter_mut() {
                    match i.next(p) {
                        Ok(_) => return Ok(()),
                        Err(mut e) => es.append(&mut e),
                    }
                }
                Err(es)
            }
            Call::Any(s) => {
                let mut is_end = false;
                let mut is_ok = false;
                let mut es = Vec::new();
                s.retain_mut(|s| match s.next(p) {
                    Ok(_) => {
                        is_ok = true;
                        true
                    }
                    Err(e) => {
                        is_end |= e.is_empty();
                        es.extend(e);
                        false
                    }
                });
                if is_ok {
                    Ok(())
                } else if is_end {
                    Err(Vec::new())
                } else {
                    Err(es)
                }
            }
        }
    }
}

/// Types convertible to [`Call`].
pub trait ToCall {
    fn to_call(&self) -> Call;
}

impl<T: ?Sized + ToCall> ToCall for &T {
    fn to_call(&self) -> Call {
        T::to_call(self)
    }
}

impl ToCall for Call {
    fn to_call(&self) -> Call {
        self.clone()
    }
}

/// Equivalent to [`Call::id`].
impl ToCall for str {
    fn to_call(&self) -> Call {
        Call::id(self)
    }
}

/// Equivalent to [`Call::id`].
impl ToCall for String {
    fn to_call(&self) -> Call {
        Call::id(self)
    }
}

/// Equivalent to [`Call::id`].
impl ToCall for usize {
    fn to_call(&self) -> Call {
        Call::id(self)
    }
}

/// Equivalent to [`Call::seq`].
impl<T: ToCall> ToCall for [T] {
    fn to_call(&self) -> Call {
        Call::seq(self)
    }
}

/// Equivalent to [`Call::seq`].
impl<T: ToCall, const N: usize> ToCall for [T; N] {
    fn to_call(&self) -> Call {
        Call::seq(self)
    }
}

/// Equivalent to [`Call::seq`].
impl<T: ToCall> ToCall for Vec<T> {
    fn to_call(&self) -> Call {
        Call::seq(self)
    }
}

/// Equivalent to [`Call::empty`].
impl ToCall for () {
    fn to_call(&self) -> Call {
        Call::empty()
    }
}

/// The error type representing that the call to [`call`] is different from what was expected.
#[derive(Debug)]
struct CallMismatchError {
    msg: String,
    actual: Records,
    expect: Vec<String>,
    mismatch_index: usize,
    thread_id: ThreadId,
}
impl CallMismatchError {
    fn new(expect: Vec<String>, mismatch_index: usize) -> Self {
        Self {
            msg: String::new(),
            actual: Records::empty(),
            expect,
            mismatch_index,
            thread_id: thread::current().id(),
        }
    }

    fn actual_id(&self, index: usize) -> &str {
        if let Some(a) = self.actual.0.get(index) {
            &a.id
        } else {
            "(end)"
        }
    }
    #[cfg(test)]
    fn set_dummy_file_line(&mut self) {
        for a in &mut self.actual.0 {
            a.set_dummy_file_line();
        }
    }

    pub fn display(&self, backtrace: bool, color: bool) -> impl Display + '_ {
        struct CallMismatchErrorDisplay<'a> {
            this: &'a CallMismatchError,
            backtrace: bool,
            color: bool,
        }
        impl std::fmt::Display for CallMismatchErrorDisplay<'_> {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                self.this.fmt_with(f, self.backtrace, self.color)
            }
        }
        CallMismatchErrorDisplay {
            this: self,
            backtrace,
            color,
        }
    }

    fn fmt_with(
        &self,
        f: &mut std::fmt::Formatter<'_>,
        backtrace: bool,
        color: bool,
    ) -> std::fmt::Result {
        let around = 5;
        if backtrace && self.actual.has_bakctrace() {
            writeln!(f, "actual calls with backtrace :")?;
            self.actual.fmt_backtrace(f, self.mismatch_index, around)?;
            writeln!(f)?;
        }

        writeln!(f, "actual calls :")?;
        self.actual
            .fmt_summary(f, self.mismatch_index, around, color)?;

        writeln!(f)?;
        writeln!(f, "{}", self.msg)?;
        if let Some(a) = self.actual.0.get(self.mismatch_index) {
            writeln!(f, "{}:{}", a.file, a.line)?;
        }
        if backtrace {
            writeln!(f, "thread : {:?}", self.thread_id)?;
        }
        writeln!(f, "actual : {}", self.actual_id(self.mismatch_index))?;
        writeln!(f, "expect : {}", self.expect.join(", "))?;
        Ok(())
    }
}
impl Display for CallMismatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.fmt_with(f, false, false)
    }
}
impl Error for CallMismatchError {}

/// Record of one [`call`] call.
#[derive(Debug)]
struct Record {
    id: String,
    file: &'static str,
    line: u32,
    backtrace: Backtrace,
    thread_id: ThreadId,
}
impl Record {
    #[cfg(test)]
    fn set_dummy_file_line(&mut self) {
        self.file = r"tests\test.rs";
        self.line = 10;
    }
}

impl Display for Record {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "# {}", self.id)?;
        writeln!(f, "{}:{}", self.file, self.line)?;
        if self.backtrace.status() == BacktraceStatus::Captured {
            writeln!(f)?;
            writeln!(f, "{}", self.backtrace)?;
        }
        Ok(())
    }
}
