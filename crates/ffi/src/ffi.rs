use std::{ io, path::PathBuf};

use thiserror::Error;

use crate::tasks::{Task, TaskContext, TaskError};

#[cxx::bridge(namespace = "task::ffi")]
pub mod detail {

    #[derive(Debug, Default, Copy, Clone)]
    struct TaskProgress {
        pub progress: u64,
        pub maximum: u64,
    }

    unsafe extern "C++" {
        include!("tokio-ffi-test/src/ffi.h");

        type CxxAny;

        type VoidFunctor;
        fn call(self: &VoidFunctor) -> Result<()>;
        type TaskFunctor;
        fn call(self: &TaskFunctor, ctx: &TaskContext) -> Result<()>;
    }

    extern "Rust" {
        type TaskContext;
        fn set_progress_maximum(&self, max: u64);
        fn update(&self, value: u64);
        fn set_progress(&self, value: u64);
        fn is_cancelled(&self) -> bool;
    }
}

#[derive(Error, Debug)]
pub enum FfiError {
    #[error("error processing request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("exception in task: {0}")]
    Functor(#[from] cxx::Exception),
    #[error("error creating file '{0}' : {1}")]
    CreateFile(PathBuf, io::Error),
    #[error("error writing to file '{0}' : {1}")]
    FileWrite(PathBuf, io::Error),
    #[error("{0}")]
    TaskError(#[from] TaskError),
}

type TaskFile = Task<PathBuf>;
type TaskBytes = Task<Vec<u8>>;
