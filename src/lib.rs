//! A small utility for testing to verify that the expected parts of the code are called in the expected order.
//!
//! See [`CallRecorder`] for details.
//!
use std::{cmp::min, collections::VecDeque, error::Error, fmt::Display};

use thread::{Global, Local, Thread};
use yansi::{Condition, Paint};

pub mod thread;

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
        $crate::CallRecord::record(::std::format!($($id)*), ::std::file!(), ::std::line!());
    };
}

/// Records and verifies calls to [`call`].
///
/// ## Example
///
/// ```should_panic
/// use assert_call::{call, CallRecorder};
///
/// let mut c = CallRecorder::new();
///
/// call!("1");
/// call!("2");
///
/// c.verify(["1", "3"]);
/// ```
///
/// The above code panics and outputs the following message
/// because the call to [`call`] macro is different from what is specified in [`verify`](CallRecorder::verify).
///
/// ```txt
/// actual calls :
///   1
/// * 2
///   (end)
///
/// mismatch call
/// src\lib.rs:10
/// actual : 2
/// expect : 3
/// ```
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
    pub fn verify(&mut self, expect: impl Into<Call>) {
        self.verify_with_msg(expect, "mismatch call");
    }

    /// Panic with specified message if [`call`] call does not match the expected pattern.
    ///
    /// Calling this method clears the recorded [`call`] calls.
    #[track_caller]
    pub fn verify_with_msg(&mut self, expect: impl Into<Call>, msg: &str) {
        match self.result_with_msg(expect, msg) {
            Ok(_) => {}
            Err(e) => {
                if Condition::tty_and_color() {
                    panic!("{e:#}");
                } else {
                    panic!("{e}")
                }
            }
        }
    }

    /// Return `Err` with specified message if [`call`] call does not match the expected pattern.
    ///
    /// Calling this method clears the recorded [`call`] calls.
    fn result_with_msg(
        &mut self,
        expect: impl Into<Call>,
        msg: &str,
    ) -> Result<(), CallMismatchError> {
        let expect: Call = expect.into();
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
#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub enum Call {
    Id(String),
    Seq(VecDeque<Call>),
    Par(Vec<Call>),
    Any(Vec<Call>),
}

impl Call {
    /// Create `Call` to represent a single [`call`] call.
    pub fn id(id: impl Display) -> Self {
        Self::Id(id.to_string())
    }

    /// Create `Call` to represent no [`call`] call.
    pub fn empty() -> Self {
        Self::Seq(VecDeque::new())
    }

    /// Create `Call` to represent all specified `Call`s will be called in sequence.
    pub fn seq<T: Into<Call>>(p: impl IntoIterator<Item = T>) -> Self {
        Self::Seq(p.into_iter().map(|x| x.into()).collect())
    }

    /// Create `Call` to represent all specified `Call`s will be called in parallel.
    pub fn par<T: Into<Call>>(p: impl IntoIterator<Item = T>) -> Self {
        Self::Par(p.into_iter().map(|x| x.into()).collect())
    }

    /// Create `Call` to represent one of the specified `Call`s will be called.
    pub fn any<T: Into<Call>>(p: impl IntoIterator<Item = T>) -> Self {
        Self::Any(p.into_iter().map(|x| x.into()).collect())
    }

    fn verify(mut self, actual: Vec<CallRecord>, msg: &str) -> Result<(), CallMismatchError> {
        match self.verify_nexts(&actual) {
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
    fn verify_nexts(&mut self, actual: &[CallRecord]) -> Result<(), CallMismatchError> {
        for index in 0..=actual.len() {
            self.verify_next(index, actual.get(index))?;
        }
        Ok(())
    }
    fn verify_next(
        &mut self,
        index: usize,
        a: Option<&CallRecord>,
    ) -> Result<(), CallMismatchError> {
        if let Err(e) = self.next(a) {
            if a.is_none() && e.is_empty() {
                return Ok(());
            }
            Err(CallMismatchError::new(e, index))
        } else {
            Ok(())
        }
    }

    fn next(&mut self, p: Option<&CallRecord>) -> Result<(), Vec<String>> {
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

/// Equivalent to [`Call::id`].
impl From<&str> for Call {
    fn from(value: &str) -> Self {
        Call::id(value)
    }
}

/// Equivalent to [`Call::id`].
impl From<String> for Call {
    fn from(value: String) -> Self {
        Call::id(value)
    }
}

/// Equivalent to [`Call::id`].
impl From<usize> for Call {
    fn from(value: usize) -> Self {
        Call::id(value)
    }
}

/// Equivalent to [`Call::seq`].
impl<T: Into<Call>, const N: usize> From<[T; N]> for Call {
    fn from(value: [T; N]) -> Self {
        Call::seq(value)
    }
}

/// Equivalent to [`Call::seq`].
impl<T: Into<Call>> From<Vec<T>> for Call {
    fn from(value: Vec<T>) -> Self {
        Call::seq(value)
    }
}

/// Equivalent to [`Call::empty`].
impl From<()> for Call {
    fn from(_: ()) -> Self {
        Call::empty()
    }
}
impl From<&Call> for Call {
    fn from(value: &Call) -> Self {
        value.clone()
    }
}

/// The error type representing that the call to [`call`] is different from what was expected.
#[derive(Debug)]
struct CallMismatchError {
    msg: String,
    actual: Vec<CallRecord>,
    expect: Vec<String>,
    mismatch_index: usize,
}
impl CallMismatchError {
    fn new(expect: Vec<String>, mismatch_index: usize) -> Self {
        Self {
            msg: String::new(),
            actual: Vec::new(),
            expect,
            mismatch_index,
        }
    }

    fn actual_id(&self, index: usize) -> &str {
        if let Some(a) = self.actual.get(index) {
            &a.id
        } else {
            "(end)"
        }
    }
    #[cfg(test)]
    fn set_dummy_file_line(&mut self) {
        for a in &mut self.actual {
            a.set_dummy_file_line();
        }
    }
}
impl Display for CallMismatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "actual calls :")?;
        let around = 5;

        let mut start = 0;
        let end = self.actual.len();

        if self.mismatch_index > around {
            start = self.mismatch_index - around;
        }
        if start > 0 {
            writeln!(f, "  ...(previous {start} calls omitted)")?;
        }
        let end = min(self.mismatch_index + around + 1, end);

        let write_actual = |f: &mut std::fmt::Formatter<'_>, index: usize, id: &str| {
            let is_mismatch = index == self.mismatch_index;
            let head = if is_mismatch { "*" } else { " " };
            let cond = if is_mismatch && f.alternate() {
                Condition::ALWAYS
            } else {
                Condition::NEVER
            };
            writeln!(f, "{}", format_args!("{head} {id}").red().whenever(cond))
        };

        for index in start..end {
            write_actual(f, index, self.actual_id(index))?;
        }
        if end == self.actual.len() {
            write_actual(f, end, "(end)")?;
        } else {
            writeln!(
                f,
                "  ...(following {} calls omitted)",
                self.actual.len() - end
            )?;
        }

        writeln!(f)?;
        writeln!(f, "{}", self.msg)?;
        if let Some(a) = self.actual.get(self.mismatch_index) {
            writeln!(f, "{}:{}", a.file, a.line)?;
        }
        writeln!(f, "actual : {}", self.actual_id(self.mismatch_index))?;
        writeln!(f, "expect : {}", self.expect.join(", "))?;
        Ok(())
    }
}
impl Error for CallMismatchError {}

/// Record of one [`call`] call.
#[derive(Debug)]
pub struct CallRecord {
    id: String,
    file: &'static str,
    line: u32,
}
impl CallRecord {
    #[cfg(test)]
    fn set_dummy_file_line(&mut self) {
        self.file = r"tests\test.rs";
        self.line = 10;
    }
}
