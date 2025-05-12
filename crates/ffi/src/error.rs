use std::fmt::Display;
use thiserror::Error;

use enum_as_inner::EnumAsInner;


#[derive(Error, Debug, EnumAsInner)]
pub enum SpawnError {
    #[error("lock failed: {0}")]
    LockError(String),
    #[error("task already awaited")]
    TaskAwaited,
    #[error("task spawn error: {0}")]
    Error(String),
}

#[derive(Error, Debug)]
#[error("Error {etype} '{path:?}': {source}")]
pub struct IoError {
    pub path: std::ffi::OsString,
    pub etype: IoErrorType,
    #[source]
    pub source: std::io::Error,
}

#[derive(Debug, Copy, Clone, EnumAsInner)]
pub enum IoErrorType {
    Read,
    Write,
    Create,
}

impl Display for IoErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Read => write!(f, "Reading"),
            Self::Write => write!(f, "Writing"),
            Self::Create => write!(f, "Creating"),
        }
    }
}

impl IoError {
    pub fn path(&self) -> std::ffi::OsString {
        self.path.clone()
    }

    pub fn etype(&self) -> IoErrorType {
        self.etype
    }
}

#[derive(Error, Debug)]
#[error("Error {code}: {msg}")]
pub struct InnerError {
    code: u32,
    msg: String,
}

#[derive(Error, Debug, EnumAsInner)]
pub enum RunError {
    #[error("error processing request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error(transparent)]
    Io(#[from] IoError),
    #[error(transparent)]
    Inner(#[from] InnerError),
}

#[derive(Error, Debug, EnumAsInner)]
pub enum TaskError {
    /// Task was canceled
    #[error("task cancelled")]
    Cancelled,
    /// Result of tokio::task::JoinHandle::abort or tokio::task::AbortHandle::abort
    #[error("task aborted by runtime")]
    Aborted,
    /// Task panic caught by runtime
    #[error("task paniced: {0}")]
    Panic(tokio::task::JoinError),
    /// Task failed to spawn child task
    #[error(transparent)]
    SpawnError(#[from] SpawnError),
    /// Task had an internal error
    #[error(transparent)]
    Error(#[from] RunError),
}
