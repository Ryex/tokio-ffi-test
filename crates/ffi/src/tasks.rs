use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

use thiserror::Error;

use tokio::{
    sync::watch::{Receiver, Sender},
    task::{AbortHandle, JoinHandle},
};

use crate::ffi::{FfiError, detail::TaskProgress};

#[derive(Error, Debug)]
pub enum TaskError {
    #[error("task cancelled")]
    TaskCancelled,
    #[error("task already awaited")]
    TaskAwaited,
    #[error("task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
}

#[derive(Debug, Clone)]
pub struct TaskMetadata {
    pub id: tokio::task::Id,
    pub abort_handle: tokio::task::AbortHandle,
    pub progress: Option<Receiver<TaskProgress>>,
    pub name: Option<String>,
    pub tags: HashSet<String>,
    pub context: Arc<TaskContext>,
}

#[derive(Debug, Default, Clone)]
pub struct TaskOptions {
    name: Option<String>,
    tags: HashSet<String>,
    progress: Option<Receiver<TaskProgress>>,
}

#[derive(Debug, Default)]
pub struct TaskContext {
    progress: AtomicU64,
    progress_maximum: AtomicU64,
    progress_sender: Option<Sender<TaskProgress>>,
    cancel: AtomicBool,
}

#[derive(Clone)]
pub struct TaskManager {
    runtime: Arc<tokio::runtime::Runtime>,
    tasks: Arc<Mutex<HashMap<tokio::task::Id, Arc<TaskMetadata>>>>,
}

impl Default for TaskManager {
    fn default() -> Self {
        TaskManager {
            runtime: Arc::new(
                tokio::runtime::Builder::new_multi_thread()
                    .thread_name_fn(|| {
                        static ATOMIC_ID: AtomicUsize = AtomicUsize::new(0);
                        let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                        format!("task-api-{}", id)
                    })
                    .enable_all()
                    .build()
                    .unwrap(),
            ),
            tasks: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl TaskManager {
    pub fn new() -> Self {
        TaskManager::default()
    }

    /// Spawn a task without recording it
    pub fn spawn_raw<T, F>(&self, fut: F) -> JoinHandle<T>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        self.runtime.spawn(fut)
    }

    /// Spawn a blocking task without recording it
    pub fn spawn_blocking_raw<T, F>(&self, fun: F) -> JoinHandle<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        self.runtime.spawn_blocking(fun)
    }

    /// spawn a future and record it's task
    pub fn spawn<T, F>(
        &self,
        fut: F,
        ctx: Arc<TaskContext>,
        options: Option<TaskOptions>,
    ) -> JoinHandle<T>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let mut tasks = self.tasks.lock().expect("task manager tasks lock poisoned");
        let task = self.runtime.spawn(fut);
        let options = options.unwrap_or_default();
        let meta = TaskMetadata {
            id: task.id(),
            abort_handle: task.abort_handle(),
            context: ctx,
            progress: options.progress,
            name: options.name,
            tags: options.tags,
        };
        tasks.insert(meta.id, Arc::new(meta));
        task
    }

    pub fn spawn_blocking<T, F>(
        &self,
        fun: F,
        ctx: Arc<TaskContext>,
        options: Option<TaskOptions>,
    ) -> JoinHandle<T>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        let mut tasks = self.tasks.lock().expect("task manager tasks lock poisoned");
        let task = self.runtime.spawn_blocking(fun);
        let options = options.unwrap_or_default();
        let meta = TaskMetadata {
            id: task.id(),
            abort_handle: task.abort_handle(),
            context: ctx,
            progress: options.progress,
            name: options.name,
            tags: options.tags,
        };
        tasks.insert(meta.id, Arc::new(meta));
        task
    }
}

impl TaskContext {
    pub fn new() -> Arc<Self> {
        Arc::new(TaskContext {
            progress: AtomicU64::new(0),
            progress_maximum: AtomicU64::new(0),
            progress_sender: None,
            cancel: AtomicBool::new(false),
        })
    }

    pub fn with_progress(sender: Sender<TaskProgress>) -> Arc<Self> {
        Arc::new(TaskContext {
            progress: AtomicU64::new(0),
            progress_maximum: AtomicU64::new(0),
            progress_sender: Some(sender),
            cancel: AtomicBool::new(false),
        })
    }

    fn send_update(&self) {
        if let Some(progress) = &self.progress_sender {
            let _ = progress.send(TaskProgress {
                progress: self.progress(),
                maximum: self.progress_maximum(),
            });
        }
    }

    pub fn progress(&self) -> u64 {
        self.progress.load(std::sync::atomic::Ordering::SeqCst)
    }

    pub fn set_progress(&self, value: u64) {
        self.progress.store(value, Ordering::SeqCst);
        self.send_update();
    }

    pub fn update(&self, value: u64) {
        self.progress.fetch_add(value, Ordering::SeqCst);
        self.send_update();
    }

    pub fn progress_maximum(&self) -> u64 {
        self.progress_maximum.load(Ordering::SeqCst)
    }

    pub fn set_progress_maximum(&self, max: u64) {
        self.progress_maximum.store(max, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

impl TaskOptions {
    pub fn new() -> Self {
        TaskOptions::default()
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_tags(mut self, tags: impl IntoIterator<Item = String>) -> Self {
        self.tags.clear();
        self.tags.extend(tags);
        self
    }

    pub fn with_progress(mut self, progress: Receiver<TaskProgress>) -> Self {
        self.progress = Some(progress);
        self
    }
}

enum TaskHandle<T: Send + 'static, E: Send + 'static> {
    Owned(JoinHandle<Result<T, E>>),
    Continued(AbortHandle),
}

impl<T: Send + 'static, E: Send + 'static> TaskHandle<T, E> {
    pub fn continuation(self) -> Result<(Self, JoinHandle<Result<T, E>>), TaskError> {
        match self {
            Self::Continued(_) => Err(TaskError::TaskAwaited),
            Self::Owned(handle) => {
                let abort = handle.abort_handle();
                Ok((Self::Continued(abort), handle))
            }
        }
    }

    pub fn is_finished(&self) -> bool {
        match self {
            Self::Continued(t) => t.is_finished(),
            Self::Owned(t) => t.is_finished(),
        }
    }
}

pub struct Task<T: Send + 'static> {
    manager: Arc<TaskManager>,
    handle: Mutex<Option<TaskHandle<T, FfiError>>>,
    ctx: Arc<TaskContext>,
}

impl<T: Send + 'static> Task<T> {
    pub fn new<F, R>(manager: &Arc<TaskManager>, f: F, options: Option<TaskOptions>) -> Box<Self>
    where
        F: FnOnce(Arc<TaskContext>) -> R,
        R: Future<Output = Result<T, FfiError>> + Send + 'static,
    {
        let tc = TaskContext::new();
        let ctx = tc.clone();
        let task = manager.spawn(f(tc), ctx.clone(), options);
        Box::new(Task {
            manager: manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx,
        })
    }

    pub fn blocking<F>(manager: &Arc<TaskManager>, f: F, options: Option<TaskOptions>) -> Box<Self>
    where
        F: FnOnce(Arc<TaskContext>) -> Result<T, FfiError> + Send + 'static,
    {
        let tc = TaskContext::new();
        let ctx = tc.clone();
        let task = manager.spawn_blocking(move || f(tc), ctx.clone(), options);
        Box::new(Task {
            manager: manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx,
        })
    }

    pub fn with_progress<F, R>(
        manager: &Arc<TaskManager>,
        f: F,
        options: Option<TaskOptions>,
    ) -> (Box<Self>, Receiver<TaskProgress>)
    where
        F: FnOnce(Arc<TaskContext>) -> R,
        R: Future<Output = Result<T, FfiError>> + Send + 'static,
    {
        let (ptx, prx) = tokio::sync::watch::channel(TaskProgress::default());
        let tc = TaskContext::with_progress(ptx);
        let ctx = tc.clone();
        let task = manager.spawn(
            f(tc),
            ctx.clone(),
            Some(options.unwrap_or_default().with_progress(prx.clone())),
        );
        (
            Box::new(Task {
                manager: manager.clone(),
                handle: Mutex::new(Some(TaskHandle::Owned(task))),
                ctx,
            }),
            prx,
        )
    }

    pub fn blocking_with_progress<F, R>(
        manager: &Arc<TaskManager>,
        f: F,
        options: Option<TaskOptions>,
    ) -> (Box<Self>, Receiver<TaskProgress>)
    where
        F: FnOnce(Arc<TaskContext>) -> Result<T, FfiError> + Send + 'static,
    {
        let (ptx, prx) = tokio::sync::watch::channel(TaskProgress::default());
        let tc = TaskContext::with_progress(ptx);
        let ctx = tc.clone();
        let task = manager.spawn_blocking(
            move || f(tc),
            ctx.clone(),
            Some(options.unwrap_or_default().with_progress(prx.clone())),
        );
        (
            Box::new(Task {
                manager: manager.clone(),
                handle: Mutex::new(Some(TaskHandle::Owned(task))),
                ctx,
            }),
            prx,
        )
    }

    pub fn then<T2, F1, F2>(
        &self,
        callback: F1,
        onerror: F2,
        options: Option<TaskOptions>,
    ) -> Result<Box<Task<T2>>, TaskError>
    where
        T2: Send + 'static,
        F1: FnOnce(T, &TaskContext) -> Result<T2, FfiError> + Send + 'static,
        F2: FnOnce(FfiError, &TaskContext) -> Result<T2, FfiError> + Send + 'static,
    {
        let handle = {
            let mut handle_lock = self.handle.lock().expect("task handle mutex poisoned");
            let (continued, handle) = handle_lock
                .take()
                .expect("task handle missing, panic during spawn?")
                .continuation()?;
            *handle_lock = Some(continued);
            handle
        };
        let (ptx, prx) = tokio::sync::watch::channel(TaskProgress::default());
        let tc = TaskContext::with_progress(ptx);
        let ctx = tc.clone();
        Ok(Box::new(Task {
            manager: self.manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(self.manager.spawn(
                async move {
                    let res = handle.await.map_err(Into::<TaskError>::into)?;
                    match res {
                        Ok(t) => tokio::task::spawn_blocking(move || callback(t, &tc))
                            .await
                            .map_err(Into::<TaskError>::into)?,
                        Err(err) => tokio::task::spawn_blocking(move || onerror(err, &tc))
                            .await
                            .map_err(Into::<TaskError>::into)?,
                    }
                },
                ctx.clone(),
                Some(options.unwrap_or_default().with_progress(prx.clone())),
            )))),
            ctx,
        }))
    }

    pub fn then_with_progress<T2, F1, F2>(
        &self,
        callback: F1,
        onerror: F2,
        options: Option<TaskOptions>,
    ) -> Result<Box<Task<T2>>, TaskError>
    where
        T2: Send + 'static,
        F1: FnOnce(T, &TaskContext) -> Result<T2, FfiError> + Send + 'static,
        F2: FnOnce(FfiError, &TaskContext) -> Result<T2, FfiError> + Send + 'static,
    {
        let handle = {
            let mut handle_lock = self.handle.lock().expect("task handle mutex poisoned");
            let (continued, handle) = handle_lock
                .take()
                .expect("task handle missing, panic during spawn?")
                .continuation()?;
            *handle_lock = Some(continued);
            handle
        };
        let tc = TaskContext::new();
        let ctx = tc.clone();
        Ok(Box::new(Task {
            manager: self.manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(self.manager.spawn(
                async move {
                    let res = handle.await.map_err(Into::<TaskError>::into)?;
                    match res {
                        Ok(t) => tokio::task::spawn_blocking(move || callback(t, &tc))
                            .await
                            .map_err(Into::<TaskError>::into)?,
                        Err(err) => tokio::task::spawn_blocking(move || onerror(err, &tc))
                            .await
                            .map_err(Into::<TaskError>::into)?,
                    }
                },
                ctx.clone(),
                options,
            )))),
            ctx,
        }))
    }

    pub fn is_finished(&self) -> bool {
        self.handle
            .lock()
            .expect("task handle mutex poisoned")
            .as_ref()
            .is_some_and(|handle| handle.is_finished())
    }

    pub fn cancel(&self) {
        self.ctx.cancel();
    }

    pub fn is_cancelled(&self) -> bool {
        self.ctx.is_cancelled()
    }
}
