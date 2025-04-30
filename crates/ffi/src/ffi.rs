use std::{io, path::PathBuf, sync::Arc};

use cxx::UniquePtr;
use detail::{
    AnyFunctor, AnyFunctorAny, AnyFunctorAnyCtx, AnyFunctorCtx, AnyFunctorError,
    AnyFunctorErrorCtx, CxxAny, TaskCxxAnyWProgress, TaskProgress, TaskVoidWProgress, VoidFunctor,
    VoidFunctorAny, VoidFunctorAnyCtx, VoidFunctorCtx, VoidFunctorError, VoidFunctorErrorCtx,
    VoidFunctorProgress,
};
use thiserror::Error;
use tokio::{sync::watch::Receiver, task::AbortHandle};

use crate::tasks::{Task, TaskContext, TaskError, TaskManager, TaskOptions};

#[cxx::bridge(namespace = "task::ffi")]
pub mod detail {

    #[derive(Debug, Default, Copy, Clone)]
    struct TaskProgress {
        pub progress: u64,
        pub maximum: u64,
    }

    #[derive(Debug)]
    struct TaskOptions {
        pub name: UniquePtr<CxxString>,
        pub tags: UniquePtr<CxxVector<CxxString>>,
        pub use_progress: bool,
    }

    struct TaskVoidWProgress {
        pub task: Box<TaskVoid>,
        pub progress: Box<ProgressReceiver>,
    }

    struct TaskCxxAnyWProgress {
        pub task: Box<TaskCxxAny>,
        pub progress: Box<ProgressReceiver>,
    }

    unsafe extern "C++" {
        include!("tokio-ffi-test/src/ffi.h");

        type CxxAny;

        type VoidFunctor;
        fn call(self: &VoidFunctor) -> Result<()>;
        type VoidFunctorCtx;
        fn call(self: &VoidFunctorCtx, ctx: &TaskContext) -> Result<()>;
        type VoidFunctorAny;
        fn call(self: &VoidFunctorAny, value: UniquePtr<CxxAny>) -> Result<()>;
        type VoidFunctorAnyCtx;
        fn call(
            self: &VoidFunctorAnyCtx,
            value: UniquePtr<CxxAny>,
            ctx: &TaskContext,
        ) -> Result<()>;
        type VoidFunctorError;
        fn call(self: &VoidFunctorError, err: &FfiError) -> Result<()>;
        type VoidFunctorErrorCtx;
        fn call(self: &VoidFunctorErrorCtx, err: &FfiError, ctx: &TaskContext) -> Result<()>;

        type AnyFunctor;
        fn call(self: &AnyFunctor) -> Result<UniquePtr<CxxAny>>;
        type AnyFunctorCtx;
        fn call(self: &AnyFunctorCtx, ctx: &TaskContext) -> Result<UniquePtr<CxxAny>>;
        type AnyFunctorAny;
        fn call(self: &AnyFunctorAny, value: UniquePtr<CxxAny>) -> Result<UniquePtr<CxxAny>>;
        type AnyFunctorAnyCtx;
        fn call(
            self: &AnyFunctorAnyCtx,
            value: UniquePtr<CxxAny>,
            ctx: &TaskContext,
        ) -> Result<UniquePtr<CxxAny>>;
        type AnyFunctorError;
        fn call(self: &AnyFunctorError, err: &FfiError) -> Result<UniquePtr<CxxAny>>;
        type AnyFunctorErrorCtx;
        fn call(
            self: &AnyFunctorErrorCtx,
            err: &FfiError,
            ctx: &TaskContext,
        ) -> Result<UniquePtr<CxxAny>>;

        type VoidFunctorProgress;
        fn call(self: &VoidFunctorProgress, progress: &TaskProgress) -> Result<()>;
    }

    extern "Rust" {
        #[cxx_name = "Error"]
        type FfiError;
        fn what(&self) -> String;
    }

    extern "Rust" {
        type TaskContext;
        fn set_progress_maximum(&self, max: u64);
        fn update(&self, value: u64);
        fn set_progress(&self, value: u64);
        fn is_cancelled(&self) -> bool;
    }

    extern "Rust" {
        type TaskVoid;

        // void funcs

        #[rust_name = then_void_ffi]
        fn then_void(
            &self,
            func: UniquePtr<VoidFunctor>,
            errfunc: UniquePtr<VoidFunctorError>,
            options: TaskOptions,
        ) -> Result<Box<TaskVoid>>;

        #[rust_name = then_void_ctx_ffi]
        fn then_void_ctx(
            &self,
            func: UniquePtr<VoidFunctorCtx>,
            errfunc: UniquePtr<VoidFunctorErrorCtx>,
            options: TaskOptions,
        ) -> Result<TaskVoidWProgress>;

        // any funcs

        #[rust_name = then_any_ffi]
        fn then_any(
            &self,
            func: UniquePtr<AnyFunctor>,
            errfunc: UniquePtr<AnyFunctorError>,
            options: TaskOptions,
        ) -> Result<Box<TaskCxxAny>>;

        #[rust_name = then_any_ctx_ffi]
        fn then_any_ctx(
            &self,
            func: UniquePtr<AnyFunctorCtx>,
            errfunc: UniquePtr<AnyFunctorErrorCtx>,
            options: TaskOptions,
        ) -> Result<TaskCxxAnyWProgress>;

    }

    extern "Rust" {
        type TaskCxxAny;

        // void funcs

        #[rust_name = then_void_ffi]
        fn then_void(
            &self,
            func: UniquePtr<VoidFunctor>,
            errfunc: UniquePtr<VoidFunctorError>,
            options: TaskOptions,
        ) -> Result<Box<TaskVoid>>;

        #[rust_name = then_void_ctx_ffi]
        fn then_void_ctx(
            &self,
            func: UniquePtr<VoidFunctorCtx>,
            errfunc: UniquePtr<VoidFunctorErrorCtx>,
            options: TaskOptions,
        ) -> Result<TaskVoidWProgress>;


        #[rust_name = then_void_any_ffi]
        fn then_void_any(
            &self,
            func: UniquePtr<VoidFunctorAny>,
            errfunc: UniquePtr<VoidFunctorError>,
            options: TaskOptions,
        ) -> Result<Box<TaskVoid>>;

        #[rust_name = then_void_any_ctx_ffi]
        fn then_void_any_ctx(
            &self,
            func: UniquePtr<VoidFunctorAnyCtx>,
            errfunc: UniquePtr<VoidFunctorErrorCtx>,
            options: TaskOptions,
        ) -> Result<TaskVoidWProgress>;

        // any funcs

        #[rust_name = then_any_ffi]
        fn then_any(
            &self,
            func: UniquePtr<AnyFunctor>,
            errfunc: UniquePtr<AnyFunctorError>,
            options: TaskOptions,
        ) -> Result<Box<TaskCxxAny>>;

        #[rust_name = then_any_any_ffi]
        fn then_any_any(
            &self,
            func: UniquePtr<AnyFunctorAny>,
            errfunc: UniquePtr<AnyFunctorError>,
            options: TaskOptions,
        ) -> Result<Box<TaskCxxAny>>;

        #[rust_name = then_any_any_ctx_ffi]
        fn then_any_any_ctx(
            &self,
            func: UniquePtr<AnyFunctorAnyCtx>,
            errfunc: UniquePtr<AnyFunctorErrorCtx>,
            options: TaskOptions,
        ) -> Result<TaskCxxAnyWProgress>;

    }

    extern "Rust" {
        type ProgressReceiver;
        fn on_progress(&self, f: UniquePtr<VoidFunctorProgress>) -> Box<ProgressAbortHandle>;
    }

    extern "Rust" {
        type ProgressAbortHandle;
        fn abort(&self);
    }

    extern "Rust" {
        type TaskManager;

        fn new_task_manager() -> Box<TaskManager>;

        #[rust_name = new_task_void_noopts_ffi]
        fn new_task_void_noopts(&self, func: UniquePtr<VoidFunctor>) -> Box<TaskVoid>;

        #[rust_name = new_task_void_ffi]
        fn new_task_void(
            &self,
            func: UniquePtr<VoidFunctor>,
            options: TaskOptions,
        ) -> Box<TaskVoid>;

        #[rust_name = new_task_void_ctx_ffi]
        fn new_task_void_ctx(
            &self,
            func: UniquePtr<VoidFunctorCtx>,
            options: TaskOptions,
        ) -> TaskVoidWProgress;

        #[rust_name = new_task_any_noopts_ffi]
        fn new_task_any_noopts(&self, func: UniquePtr<AnyFunctor>) -> Box<TaskCxxAny>;

        #[rust_name = new_task_any_ffi]
        fn new_task_any(
            &self,
            func: UniquePtr<AnyFunctor>,
            options: TaskOptions,
        ) -> Box<TaskCxxAny>;

        #[rust_name = new_task_any_ctx_ffi ]
        fn new_task_any_ctx(
            &self,
            func: UniquePtr<AnyFunctorCtx>,
            options: TaskOptions,
        ) -> TaskCxxAnyWProgress;

    }
}

/// may not technicaly be true. up to users of wrapper to never wrap a non send type
unsafe impl Send for CxxAny {}
unsafe impl Send for VoidFunctorProgress {}
unsafe impl Send for VoidFunctor {}
unsafe impl Send for VoidFunctorCtx {}
unsafe impl Send for VoidFunctorAny {}
unsafe impl Send for VoidFunctorAnyCtx {}
unsafe impl Send for VoidFunctorError {}
unsafe impl Send for VoidFunctorErrorCtx {}
unsafe impl Send for AnyFunctor {}
unsafe impl Send for AnyFunctorCtx {}
unsafe impl Send for AnyFunctorAny {}
unsafe impl Send for AnyFunctorAnyCtx {}
unsafe impl Send for AnyFunctorError {}
unsafe impl Send for AnyFunctorErrorCtx {}

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

impl FfiError {
    pub fn what(&self) -> String {
        self.to_string()
    }
}

type TaskFile = Task<PathBuf>;
type TaskBytes = Task<Vec<u8>>;
type TaskCxxAny = Task<cxx::UniquePtr<CxxAny>>;
type TaskVoid = Task<()>;

fn transform_options(options: detail::TaskOptions) -> TaskOptions {
    let name = options.name.as_ref().map(|name| name.to_string());
    let mut opts = TaskOptions::new(name);
    if let Some(tags) = options.tags.as_ref() {
        opts = opts.with_tags(tags.iter().map(|tag| tag.to_string()));
    }
    opts
}

pub struct ProgressAbortHandle(Option<AbortHandle>);

impl ProgressAbortHandle {
    pub fn abort(&self) {
        if let Some(hndl) = &self.0 {
            hndl.abort();
        }
    }
}

pub struct ProgressReceiver {
    recv: Option<Receiver<TaskProgress>>,
    runtime: Arc<tokio::runtime::Runtime>,
}

impl ProgressReceiver {
    pub fn on_progress(&self, f: cxx::UniquePtr<VoidFunctorProgress>) -> Box<ProgressAbortHandle> {
        if let Some(recv) = &self.recv {
            let mut recv = recv.clone();
            let task = self.runtime.spawn(async move {
                while (recv.changed().await).is_ok() {
                    let _ = f.call(&recv.borrow());
                }
            });
            Box::new(ProgressAbortHandle(Some(task.abort_handle())))
        } else {
            Box::new(ProgressAbortHandle(None))
        }
    }
}

fn wrap_receiver(
    recv: Option<Receiver<TaskProgress>>,
    runtime: Arc<tokio::runtime::Runtime>,
) -> Box<ProgressReceiver> {
    Box::new(ProgressReceiver { recv, runtime })
}

fn new_task_manager() -> Box<TaskManager> {
    Box::new(TaskManager::new())
}

impl TaskManager {
    fn new_task_void_noopts_ffi(&self, func: cxx::UniquePtr<VoidFunctor>) -> Box<TaskVoid> {
        self.new_blocking(move || Ok(func.call()?), None)
    }

    fn new_task_void_ffi(
        &self,
        func: cxx::UniquePtr<VoidFunctor>,
        options: detail::TaskOptions,
    ) -> Box<TaskVoid> {
        self.new_blocking(move || Ok(func.call()?), Some(transform_options(options)))
    }

    fn new_task_void_ctx_ffi(
        &self,
        func: cxx::UniquePtr<VoidFunctorCtx>,
        options: detail::TaskOptions,
    ) -> TaskVoidWProgress {
        let (task, recv) = self.new_blocking_with_progress(
            move |ctx| Ok(func.call(&ctx)?),
            Some(transform_options(options)),
        );
        TaskVoidWProgress {
            task,
            progress: wrap_receiver(Some(recv), self.runtime()),
        }
    }

    fn new_task_any_noopts_ffi(&self, func: cxx::UniquePtr<AnyFunctor>) -> Box<TaskCxxAny> {
        self.new_blocking(move || Ok(func.call()?), None)
    }

    fn new_task_any_ffi(
        &self,
        func: cxx::UniquePtr<AnyFunctor>,
        options: detail::TaskOptions,
    ) -> Box<TaskCxxAny> {
        self.new_blocking(move || Ok(func.call()?), Some(transform_options(options)))
    }

    fn new_task_any_ctx_ffi(
        &self,
        func: cxx::UniquePtr<AnyFunctorCtx>,
        options: detail::TaskOptions,
    ) -> TaskCxxAnyWProgress {
        let (task, recv) = self.new_blocking_with_progress(
            move |ctx| Ok(func.call(&ctx)?),
            Some(transform_options(options)),
        );
        TaskCxxAnyWProgress {
            task,
            progress: wrap_receiver(Some(recv), self.runtime()),
        }
    }
}

impl TaskCxxAny {
    // void funcs

    fn then_void_noopts_ffi(
        &self,
        func: cxx::UniquePtr<VoidFunctor>,
        errfunc: cxx::UniquePtr<VoidFunctorError>,
    ) -> Result<Box<TaskVoid>, FfiError> {
        let task = self.then(
            move |_| Ok(func.call()?),
            move |e| Ok(errfunc.call(&e)?),
            None,
        )?;
        Ok(task)
    }

    fn then_void_ffi(
        &self,
        func: cxx::UniquePtr<VoidFunctor>,
        errfunc: cxx::UniquePtr<VoidFunctorError>,
        options: detail::TaskOptions,
    ) -> Result<Box<TaskVoid>, FfiError> {
        let task = self.then(
            move |_| Ok(func.call()?),
            move |e| Ok(errfunc.call(&e)?),
            Some(transform_options(options)),
        )?;
        Ok(task)
    }


    fn then_void_ctx_ffi(
        &self,
        func: cxx::UniquePtr<VoidFunctorCtx>,
        errfunc: cxx::UniquePtr<VoidFunctorErrorCtx>,
        options: detail::TaskOptions,
    ) -> Result<TaskVoidWProgress, FfiError> {
        let (task, recv) = self.then_with_progress(
            move |_, ctx| Ok(func.call(ctx)?),
            move |e, ctx| Ok(errfunc.call(&e, ctx)?),
            Some(transform_options(options)),
        )?;
        Ok(TaskVoidWProgress {
            task,
            progress: wrap_receiver(Some(recv), self.manager().runtime()),
        })
    }

    fn then_void_any_ffi(
        &self,
        func: cxx::UniquePtr<VoidFunctorAny>,
        errfunc: cxx::UniquePtr<VoidFunctorError>,
        options: detail::TaskOptions,
    ) -> Result<Box<TaskVoid>, FfiError> {
        let task = self.then(
            move |v| Ok(func.call(v)?),
            move |e| Ok(errfunc.call(&e)?),
            Some(transform_options(options)),
        )?;
        Ok(task)
    }

    fn then_void_any_ctx_ffi(
        &self,
        func: cxx::UniquePtr<VoidFunctorAnyCtx>,
        errfunc: cxx::UniquePtr<VoidFunctorErrorCtx>,
        options: detail::TaskOptions,
    ) -> Result<TaskVoidWProgress, FfiError> {
        let (task, recv) = self.then_with_progress(
            move |v, ctx| Ok(func.call(v, ctx)?),
            move |e, ctx| Ok(errfunc.call(&e, ctx)?),
            Some(transform_options(options)),
        )?;
        Ok(TaskVoidWProgress {
            task,
            progress: wrap_receiver(Some(recv), self.manager().runtime()),
        })
    }

    // any funcs

    fn then_any_ffi(
        &self,
        func: cxx::UniquePtr<AnyFunctor>,
        errfunc: cxx::UniquePtr<AnyFunctorError>,
        options: detail::TaskOptions,
    ) -> Result<Box<TaskCxxAny>, FfiError> {
        let task = self.then(
            move |_| Ok(func.call()?),
            move |e| Ok(errfunc.call(&e)?),
            Some(transform_options(options)),
        )?;
        Ok(task)
    }

    fn then_any_any_ffi(
        &self,
        func: cxx::UniquePtr<AnyFunctorAny>,
        errfunc: cxx::UniquePtr<AnyFunctorError>,
        options: detail::TaskOptions,
    ) -> Result<Box<TaskCxxAny>, FfiError> {
        let task = self.then(
            move |v| Ok(func.call(v)?),
            move |e| Ok(errfunc.call(&e)?),
            Some(transform_options(options)),
        )?;
        Ok(task)
    }

    fn then_any_any_ctx_ffi(
        &self,
        func: cxx::UniquePtr<AnyFunctorAnyCtx>,
        errfunc: cxx::UniquePtr<AnyFunctorErrorCtx>,
        options: detail::TaskOptions,
    ) -> Result<TaskCxxAnyWProgress, FfiError> {
        let (task, recv) = self.then_with_progress(
            move |v, ctx| Ok(func.call(v, ctx)?),
            move |e, ctx| Ok(errfunc.call(&e, ctx)?),
            Some(transform_options(options)),
        )?;
        Ok(TaskCxxAnyWProgress {
            task,
            progress: wrap_receiver(Some(recv), self.manager().runtime()),
        })
    }
}

impl TaskVoid {
    // void funcs

    fn then_void_noopts_ffi(
        &self,
        func: cxx::UniquePtr<VoidFunctor>,
        errfunc: cxx::UniquePtr<VoidFunctorError>,
    ) -> Result<Box<TaskVoid>, FfiError> {
        let task = self.then(
            move |_| Ok(func.call()?),
            move |e| Ok(errfunc.call(&e)?),
            None,
        )?;
        Ok(task)
    }

    fn then_void_ffi(
        &self,
        func: cxx::UniquePtr<VoidFunctor>,
        errfunc: cxx::UniquePtr<VoidFunctorError>,
        options: detail::TaskOptions,
    ) -> Result<Box<TaskVoid>, FfiError> {
        let task = self.then(
            move |_| Ok(func.call()?),
            move |e| Ok(errfunc.call(&e)?),
            Some(transform_options(options)),
        )?;
        Ok(task)
    }

    fn then_void_ctx_ffi(
        &self,
        func: cxx::UniquePtr<VoidFunctorCtx>,
        errfunc: cxx::UniquePtr<VoidFunctorErrorCtx>,
        options: detail::TaskOptions,
    ) -> Result<TaskVoidWProgress, FfiError> {
        let (task, recv) = self.then_with_progress(
            move |_, ctx| Ok(func.call(ctx)?),
            move |e, ctx| Ok(errfunc.call(&e, ctx)?),
            Some(transform_options(options)),
        )?;
        Ok(TaskVoidWProgress {
            task,
            progress: wrap_receiver(Some(recv), self.manager().runtime()),
        })
    }

    // any funcs

    fn then_any_noopts_ffi(
        &self,
        func: cxx::UniquePtr<AnyFunctor>,
        errfunc: cxx::UniquePtr<AnyFunctorError>,
    ) -> Result<Box<TaskCxxAny>, FfiError> {
        let task = self.then(
            move |_| Ok(func.call()?),
            move |e| Ok(errfunc.call(&e)?),
            None,
        )?;
        Ok(task)
    }

    fn then_any_ffi(
        &self,
        func: cxx::UniquePtr<AnyFunctor>,
        errfunc: cxx::UniquePtr<AnyFunctorError>,
        options: detail::TaskOptions,
    ) -> Result<Box<TaskCxxAny>, FfiError> {
        let task = self.then(
            move |_| Ok(func.call()?),
            move |e| Ok(errfunc.call(&e)?),
            Some(transform_options(options)),
        )?;
        Ok(task)
    }

    fn then_any_ctx_ffi(
        &self,
        func: cxx::UniquePtr<AnyFunctorCtx>,
        errfunc: cxx::UniquePtr<AnyFunctorErrorCtx>,
        options: detail::TaskOptions,
    ) -> Result<TaskCxxAnyWProgress, FfiError> {
        let (task, recv) = self.then_with_progress(
            move |_, ctx| Ok(func.call(ctx)?),
            move |e, ctx| Ok(errfunc.call(&e, ctx)?),
            Some(transform_options(options)),
        )?;
        Ok(TaskCxxAnyWProgress {
            task,
            progress: wrap_receiver(Some(recv), self.manager().runtime()),
        })
    }
}
