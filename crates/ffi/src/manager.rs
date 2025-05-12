use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicI32, AtomicUsize, Ordering},
    },
};


use tokio::{
    sync::watch::{Receiver, Sender},
    task::{AbortHandle, JoinHandle},
};

use crate::{error::{SpawnError, TaskError}, task::TaskId};

#[derive(Debug, Clone)]
pub struct TaskMetadata {
    id: TaskId,
    abort_handle: tokio::task::AbortHandle,
    #[allow(dead_code)]
    progress: Option<Receiver<i32>>,
    name: Option<String>,
    tags: HashSet<String>,
    #[allow(dead_code)]
    context: Option<Arc<TaskContext>>,
}

impl TaskMetadata {
    pub fn id(&self) -> TaskId {
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
    progress: Option<Receiver<i32>>,
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
    progress: AtomicI32,
    progress_sender: Option<Sender<i32>>,
    cancel: AtomicBool,
}

impl TaskContext {
    pub fn new() -> Arc<Self> {
        Arc::new(TaskContext {
            progress: AtomicI32::new(0),
            progress_sender: None,
            cancel: AtomicBool::new(false),
        })
    }

    pub fn with_progress(sender: Sender<i32>) -> Arc<Self> {
        Arc::new(TaskContext {
            progress: AtomicI32::new(0),
            progress_sender: Some(sender),
            cancel: AtomicBool::new(false),
        })
    }

    /// send current progress on the `::tokio::task::watch` channel
    /// if it exists
    pub fn send_update(&self) {
        if let Some(progress) = &self.progress_sender {
            let _ = progress.send(self.progress());
        }
    }

    /// get current progress
    pub fn progress(&self) -> i32 {
        self.progress.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// set progress to value and send and update
    pub fn set_progress(&self, value: i32) {
        self.progress.store(value, Ordering::SeqCst);
        self.send_update();
    }

    /// add value to current progress and send an update
    pub fn update(&self, value: i32) {
        self.progress.fetch_add(value, Ordering::SeqCst);
        self.send_update();
    }

    /// has the cancel flag been set
    pub fn is_cancelled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    /// set the cancel flag
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskListing(Arc<Mutex<HashMap<TaskId, Arc<TaskMetadata>>>>);

impl TaskListing {
    pub fn new() -> Self {
        TaskListing::default()
    }

    pub fn insert(&self, id: TaskId, metadata: TaskMetadata) {
        self.0
            .lock()
            .expect("tasks listing lock poisoned")
            .insert(id, Arc::new(metadata));
    }

    pub fn remove(&self, id: TaskId) {
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
}

static TASK_MANAGER: OnceLock<Mutex<TaskManager>> = OnceLock::new();
fn task_manager() -> TaskManager {
    TASK_MANAGER
        .get_or_init(|| Mutex::new(TaskManager::default()))
        .lock()
        .expect("TaskManager lock poisoned")
        .clone()
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
    id: TaskId,
    manager: TaskManager,
    handle: Mutex<Option<TaskHandle<Result<T, TaskError>>>>,
    ctx: Option<Arc<TaskContext>>,
    progress: Option<Receiver<TaskProgress>>,
}

impl<T: Send + 'static> Task<T> {
    pub fn new<F, R>(f: F, options: Option<TaskOptions>) -> Box<Self>
    where
        F: FnOnce() -> R,
        R: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        let manager = task_manager();
        let task = manager.spawn(f(), None, options.map(TaskOptions::to_spawn_options));
        Box::new(Task {
            id: task.id(),
            manager: manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx: None,
            progress: None,
        })
    }

    pub fn blocking<F>(f: F, options: Option<TaskOptions>) -> Box<Self>
    where
        F: FnOnce() -> Result<T, TaskError> + Send + 'static,
    {
        let manager = task_manager();
        let task = manager.spawn_blocking(f, None, options.map(TaskOptions::to_spawn_options));
        Box::new(Task {
            id: task.id(),
            manager: manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx: None,
            progress: None,
        })
    }

    pub fn with_ctx<F, R>(f: F, options: Option<TaskOptions>) -> Box<Self>
    where
        F: FnOnce(&TaskContext) -> R,
        R: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        let manager = task_manager();
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
            id: task.id(),
            manager: manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx: Some(ctx),
            progress: Some(prx),
        })
    }

    pub fn blocking_with_progress<F>(
        f: F,
        options: Option<TaskOptions>,
    ) -> Box<Self>
    where
        F: FnOnce(&TaskContext) -> Result<T, TaskError> + Send + 'static,
    {
        let manager = task_manager();
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
            id: task.id(),
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
        let task = self.manager.spawn(
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
        );
        Ok(Box::new(Task {
            id: task.id(),
            manager: self.manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx: None,
            progress: None,
        }))
    }

    pub fn then_with_ctx<T2, F>(
        &self,
        callback: F,
        options: Option<TaskOptions>,
    ) -> Result<Box<Task<T2>>, SpawnError>
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
        let (ptx, prx) = tokio::sync::watch::channel(0);
        let tc = TaskContext::with_progress(ptx);
        let ctx = tc.clone();
        let task = self.manager.spawn(
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
        );
        Ok(Box::new(Task {
            id: task.id(),
            manager: self.manager.clone(),
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
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
        F: Fn(i32) + Send + 'static,
    {
        if let Some(recv) = &self.progress {
            let mut recv = recv.clone();
            let task = self.manager.spawn_raw(async move {
                while (recv.changed().await).is_ok() {
                    callback(&recv.borrow());
                }
            });
            Some(task.abort_handle())
        } else {
            None
        }
    }

    pub fn id(&self) -> TaskId {
        self.id
    }
}
