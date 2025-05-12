use std::{
    fmt::Display,
    sync::{atomic::{AtomicUsize, Ordering}, Arc},
};

use crate::{error::TaskError, manager::TaskManager};


#[repr(u8)]
pub enum TaskState {
    /// Task not yet started
    Pending,
    /// Task currently running
    Running,
    /// Task finished
    Finished,
    /// The task was requested to stop cleanly
    Canceled,
    /// The task was aborted it may or may not finish
    Aborted,
    /// Task depends on a parent which is not yet finished
    Waiting,
}

impl TaskState {
    /// used for mapping the enum to a C/C++ enum
    pub fn discriminant(&self) -> u8 {
        // SAFETY: `Self` is marked `repr(u8)` so its layout is a `repr(C)` `union`
        // of `repr(C)` structs, each with the `u8` discriminant as its first
        // field. As such, we can read the discriminant without offsetting the pointer.
        unsafe { *<*const _>::from(self).cast::<u8>() }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TaskId(usize);

impl Default for TaskId {
    fn default() -> Self {
        static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
        let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
        TaskId(id)
    }
}

impl Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "task-{:#x}", self.0)
    }
}

pub enum TaskResult<T> {
    /// Task has not returned yet
    Pending,
    /// Task finished with result
    Ready(T),
    /// Task finished with an error (cancel, panic, internal)
    Fail(TaskError),
    /// Task finished but it's result has been movd
    Moved,
}

/// Presumably passed arround as a Arc<dyn Task + Send + Sync>
/// impls should use atomics and locks for internal state
pub trait Task<T>
where
    T: Send,
{
    /// Task Id
    fn id(&self) -> TaskId;
    /// Task state
    fn state(&self) -> TaskState;
    /// should always return a value between 0 and 100
    /// tasks should internaly normalize to that range
    fn progress(&self) -> i32;
    /// attempt to fetch the result
    /// *may* move the result *out* of the task if it's ready
    /// Calling before the task is finished returns `Pending`
    /// Calling again after getting the result *may* return
    /// `Moved` depending on implimentation
    fn result(&self) -> TaskResult<T>;
    
    /// set a callback to run when task is finished
    fn on_finish(&self, func: Box<dyn Fn(&dyn Task<T>)>);

    /// Spawn task using manager
    fn start(self: Arc<Self>, manager: &TaskManager) -> tokio::task::JoinHandle<()>;

    /// Set an internal cancel flag (AtomicBool) for the task to check internaly 
    /// and return a `Fail(TaskError::Canceled)` result.
    /// also sets state to `Canceled`
    ///
    /// This is a best effort method, the task *may* still finish and return non-canceled
    /// result if it does nto check it's cancel flag.
    fn cancel(&self);
}
