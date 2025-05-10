use std::{
    collections::{HashMap, HashSet},
    io,
    path::PathBuf,
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

#[derive(Error, Debug)]
pub enum FfiError {
    #[error("error processing request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("error creating file '{0}' : {1}")]
    CreateFile(PathBuf, io::Error),
    #[error("error writing to file '{0}' : {1}")]
    FileWrite(PathBuf, io::Error),
    #[error("{0}")]
    TaskError(#[from] TaskSpawnError),
}

#[derive(Error, Debug)]
pub enum TaskSpawnError {
    #[error("task already awaited")]
    TaskAwaited,
    #[error("task spawn error: {0}")]
    Error(String),
}

#[derive(Error, Debug)]
pub enum TaskError {
    #[error("task cancelled")]
    TaskCancelled,
    #[error("task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("{0}")]
    SpanwError(#[from] TaskSpawnError),
}

#[derive(Debug, Default, Copy, Clone)]
pub struct TaskProgress {
    progress: u64,
    maximum: u64,
}

impl TaskProgress {
    pub fn progress(&self) -> u64 {
        self.progress
    }
    pub fn maximum(&self) -> u64 {
        self.maximum
    }
}

#[derive(Debug, Clone)]
pub struct TaskMetadata {
    id: tokio::task::Id,
    abort_handle: tokio::task::AbortHandle,
    #[allow(dead_code)]
    progress: Option<Receiver<TaskProgress>>,
    name: Option<String>,
    tags: HashSet<String>,
    #[allow(dead_code)]
    context: Option<Arc<TaskContext>>,
}

impl TaskMetadata {
    pub fn id(&self) -> tokio::task::Id {
        self.id
    }

    pub fn id_display(&self) -> String {
        self.id.to_string()
    }

    pub fn abort_handle(&self) -> tokio::task::AbortHandle {
        self.abort_handle.clone()
    }

    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }

    pub fn tags(&self) -> &HashSet<String> {
        &self.tags
    }
}

#[derive(Debug, Default, Clone)]
pub struct TaskSpawnOptions {
    name: Option<String>,
    tags: HashSet<String>,
    progress: Option<Receiver<TaskProgress>>,
}

#[derive(Debug, Default, Clone)]
pub struct TaskOptions {
    name: Option<String>,
    tags: HashSet<String>,
}

impl TaskOptions {
    pub fn new() -> Self {
        TaskOptions::default()
    }
    pub fn name(&self) -> Option<&String> {
        self.name.as_ref()
    }
    pub fn set_name(&mut self, name: Option<String>) -> &mut Self {
        self.name = name;
        self
    }
    pub fn tags(&self) -> &HashSet<String> {
        &self.tags
    }
    pub fn tags_mut(&mut self) -> &mut HashSet<String> {
        &mut self.tags
    }
    pub fn to_spawn_options(self) -> TaskSpawnOptions {
        TaskSpawnOptions {
            name: self.name,
            tags: self.tags,
            progress: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct TaskContext {
    progress: AtomicU64,
    progress_maximum: AtomicU64,
    progress_sender: Option<Sender<TaskProgress>>,
    cancel: AtomicBool,
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

#[derive(Debug, Clone, Default)]
pub struct TaskListing(Arc<Mutex<HashMap<tokio::task::Id, Arc<TaskMetadata>>>>);

impl TaskListing {
    pub fn new() -> Self {
        TaskListing::default()
    }

    pub fn insert(&self, id: tokio::task::Id, metadata: TaskMetadata) {
        self.0
            .lock()
            .expect("tasks listing lock poisoned")
            .insert(id, Arc::new(metadata));
    }

    pub fn remove(&self, id: tokio::task::Id) {
        self.0
            .lock()
            .expect("tasks listing lock poisoned")
            .remove(&id);
    }

    /// remove finished tasks
    pub fn clean(&self) {
        self.0
            .lock()
            .expect("tasks listing lock poisoned")
            .retain(|_, meta| !meta.abort_handle.is_finished());
    }
}

#[derive(Clone)]
pub struct TaskManager {
    runtime: Arc<tokio::runtime::Runtime>,
    tasks: TaskListing,
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
            tasks: TaskListing::new(),
        }
    }
}

impl TaskManager {
    pub fn new() -> Self {
        TaskManager::default()
    }

    pub fn runtime(&self) -> Arc<tokio::runtime::Runtime> {
        self.runtime.clone()
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
        ctx: Option<Arc<TaskContext>>,
        options: Option<TaskSpawnOptions>,
    ) -> JoinHandle<Result<T, TaskError>>
    where
        T: Send + 'static,
        F: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        let tasks = self.tasks.clone();
        let task = self.runtime.spawn(async move {
            let ret = fut.await;
            tasks.remove(tokio::task::id());
            ret
        });
        let options = options.unwrap_or_default();
        let meta = TaskMetadata {
            id: task.id(),
            abort_handle: task.abort_handle(),
            context: ctx,
            progress: options.progress,
            name: options.name,
            tags: options.tags,
        };
        self.tasks.insert(meta.id, meta);
        task
    }

    pub fn spawn_blocking<T, F>(
        &self,
        fun: F,
        ctx: Option<Arc<TaskContext>>,
        options: Option<TaskSpawnOptions>,
    ) -> JoinHandle<Result<T, TaskError>>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, TaskError> + Send + 'static,
    {
        let tasks = self.tasks.clone();
        let task = self.runtime.spawn(async move {
            let ret = tokio::task::spawn_blocking(fun)
                .await
                .map_err(Into::<TaskError>::into)
                .and_then(|ret| ret);
            tasks.remove(tokio::task::id());
            ret
        });
        let options = options.unwrap_or_default();
        let meta = TaskMetadata {
            id: task.id(),
            abort_handle: task.abort_handle(),
            context: ctx,
            progress: options.progress,
            name: options.name,
            tags: options.tags,
        };
        self.tasks.insert(meta.id, meta);
        task
    }

    pub fn new_task<T, F, R>(&self, f: F, options: Option<TaskOptions>) -> Box<Task<T>>
    where
        T: Send + 'static,
        F: FnOnce() -> R,
        R: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        Task::new(self, f, options)
    }

    pub fn new_blocking<T, F>(&self, f: F, options: Option<TaskOptions>) -> Box<Task<T>>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, TaskError> + Send + 'static,
    {
        Task::blocking(self, f, options)
    }

    pub fn new_task_with_ctx<T, F, R>(&self, f: F, options: Option<TaskOptions>) -> Box<Task<T>>
    where
        T: Send + 'static,
        F: FnOnce(&TaskContext) -> R,
        R: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        Task::with_ctx(self, f, options)
    }

    pub fn new_blocking_with_ctx<T, F>(&self, f: F, options: Option<TaskOptions>) -> Box<Task<T>>
    where
        T: Send + 'static,
        F: FnOnce(&TaskContext) -> Result<T, TaskError> + Send + 'static,
    {
        Task::blocking_with_progress(self, f, options)
    }
}

impl TaskSpawnOptions {
    pub fn new(name: Option<String>) -> Self {
        TaskSpawnOptions {
            name,
            tags: HashSet::new(),
            progress: None,
        }
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

enum TaskHandle<T: Send + 'static> {
    Owned(JoinHandle<T>),
    Continued(AbortHandle),
}

impl<T: Send + 'static> TaskHandle<T> {
    pub fn continuation(self) -> Result<(Self, JoinHandle<T>), TaskSpawnError> {
        match self {
            Self::Continued(_) => Err(TaskSpawnError::TaskAwaited),
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

    #[allow(dead_code)]
    pub fn is_continued(&self) -> bool {
        match self {
            Self::Continued(_) => true,
            Self::Owned(_) => false,
        }
    }
}

pub struct Task<T: Send + 'static> {
    manager: TaskManager,
    handle: Mutex<Option<TaskHandle<Result<T, TaskError>>>>,
    ctx: Option<Arc<TaskContext>>,
    progress: Option<Receiver<TaskProgress>>,
}

impl<T: Send + 'static> Task<T> {
    pub fn new<F, R>(manager: &TaskManager, f: F, options: Option<TaskOptions>) -> Box<Self>
    where
        F: FnOnce() -> R,
        R: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        let task = manager.spawn(f(), None, options.map(TaskOptions::to_spawn_options));
        Box::new(Task {
            manager: manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx: None,
            progress: None,
        })
    }

    pub fn blocking<F>(manager: &TaskManager, f: F, options: Option<TaskOptions>) -> Box<Self>
    where
        F: FnOnce() -> Result<T, TaskError> + Send + 'static,
    {
        let task = manager.spawn_blocking(f, None, options.map(TaskOptions::to_spawn_options));
        Box::new(Task {
            manager: manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx: None,
            progress: None,
        })
    }

    pub fn with_ctx<F, R>(
        manager: &TaskManager,
        f: F,
        options: Option<TaskOptions>,
    ) -> Box<Self>
    where
        F: FnOnce(&TaskContext) -> R,
        R: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        let (ptx, prx) = tokio::sync::watch::channel(TaskProgress::default());
        let tc = TaskContext::with_progress(ptx);
        let ctx = tc.clone();
        let task = manager.spawn(
            f(&tc),
            Some(ctx.clone()),
            Some(
                options
                    .unwrap_or_default()
                    .to_spawn_options()
                    .with_progress(prx.clone()),
            ),
        );
        Box::new(Task {
            manager: manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx: Some(ctx),
            progress: Some(prx),
        })
    }

    pub fn blocking_with_progress<F>(
        manager: &TaskManager,
        f: F,
        options: Option<TaskOptions>,
    ) -> Box<Self>
    where
        F: FnOnce(&TaskContext) -> Result<T, TaskError> + Send + 'static,
    {
        let (ptx, prx) = tokio::sync::watch::channel(TaskProgress::default());
        let tc = TaskContext::with_progress(ptx);
        let ctx = tc.clone();
        let task = manager.spawn_blocking(
            move || f(&tc),
            Some(ctx.clone()),
            Some(
                options
                    .unwrap_or_default()
                    .to_spawn_options()
                    .with_progress(prx.clone()),
            ),
        );
        Box::new(Task {
            manager: manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx: Some(ctx),
            progress: Some(prx),
        })
    }

    pub fn manager(&self) -> &TaskManager {
        &self.manager
    }

    pub fn then<T2, F>(
        &self,
        callback: F,
        options: Option<TaskOptions>,
    ) -> Result<Box<Task<T2>>, TaskSpawnError>
    where
        T2: Send + 'static,
        F: FnOnce(Result<T, TaskError>) -> Result<T2, TaskError> + Send + 'static,
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
        Ok(Box::new(Task {
            manager: self.manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(self.manager.spawn(
                async move {
                    let res = handle
                        .await
                        .map_err(Into::<TaskError>::into)
                        .and_then(|ret| ret);
                    tokio::task::spawn_blocking(move || callback(res))
                        .await
                        .map_err(Into::<TaskError>::into)
                        .and_then(|ret| ret)
                },
                None,
                options.map(TaskOptions::to_spawn_options),
            )))),
            ctx: None,
            progress: None,
        }))
    }

    pub fn then_with_ctx<T2, F>(
        &self,
        callback: F,
        options: Option<TaskOptions>,
    ) -> Result<Box<Task<T2>>, TaskSpawnError>
    where
        T2: Send + 'static,
        F: FnOnce(Result<T, TaskError>, &TaskContext) -> Result<T2, TaskError> + Send + 'static,
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
            handle: Mutex::new(Some(TaskHandle::Owned(
                self.manager.spawn(
                    async move {
                        let res = handle
                            .await
                            .map_err(Into::<TaskError>::into)
                            .and_then(|ret| ret);
                        tokio::task::spawn_blocking(move || callback(res, &tc))
                            .await
                            .map_err(Into::<TaskError>::into)
                            .and_then(|ret| ret)
                    },
                    Some(ctx.clone()),
                    Some(
                        options
                            .unwrap_or_default()
                            .to_spawn_options()
                            .with_progress(prx.clone()),
                    ),
                ),
            ))),
            ctx: Some(ctx),
            progress: Some(prx),
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
        if let Some(ctx) = &self.ctx {
            ctx.cancel();
        }
    }

    pub fn is_cancelled(&self) -> bool {
        self.ctx.as_ref().is_some_and(|ctx| ctx.is_cancelled())
    }

    pub fn on_progress<F>(&self, callback: F) -> Option<AbortHandle>
    where
        F: Fn(&TaskProgress) + Send + 'static,
    {
        if let Some(recv) = &self.progress {
            let mut recv = recv.clone();
            let task = self.manager.spawn_raw(async move {
                while (recv.changed().await).is_ok() {
                    let _ = callback(&recv.borrow());
                }
            });
            Some(task.abort_handle())
        } else {
            None
        }
    }
}
