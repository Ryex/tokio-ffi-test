use std::{
    collections::{HashMap, HashSet},
    ffi::OsString,
    fmt::{Debug, Display},
    io,
    num::NonZeroU64,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    },
};

use thiserror::Error;

use tokio::{
    sync::{
        mpsc::Sender as MSPCSender,
        watch::{Receiver as WatchReceiver, Sender as WatchSender},
    },
    task::{AbortHandle, JoinHandle},
};

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq, PartialOrd, Ord)]
pub struct TaskId(pub(crate) NonZeroU64);

pub mod current {
    use std::cell::Cell;

    use super::*;

    /// Thread Local storage context for current tasks [`Id`] and [`TaskContext`]
    struct RuntimeContext {
        current_task_id: Cell<Option<TaskId>>,
        current_task_ctx: Cell<Option<Arc<TaskContext>>>,
    }

    ::std::thread_local! {
        static CONTEXT: RuntimeContext = const {
            RuntimeContext {
                current_task_id: Cell::new(None),
                current_task_ctx: Cell::new(None),
            }
        }
    }

    pub(crate) fn set_current_task_id(id: Option<TaskId>) -> Option<TaskId> {
        CONTEXT
            .try_with(|ctx| ctx.current_task_id.replace(id))
            .unwrap_or(None)
    }

    pub(crate) fn current_task_id() -> Option<TaskId> {
        CONTEXT
            .try_with(|ctx| ctx.current_task_id.get())
            .unwrap_or(None)
    }

    pub(crate) fn set_current_task_ctx(
        task_ctx: Option<Arc<TaskContext>>,
    ) -> Option<Arc<TaskContext>> {
        CONTEXT
            .try_with(|ctx| ctx.current_task_ctx.replace(task_ctx))
            .unwrap_or(None)
    }

    pub(crate) fn current_task_ctx() -> Option<Arc<TaskContext>> {
        CONTEXT
            .try_with(|ctx| {
                let tc = ctx.current_task_ctx.take();
                let clone = tc.clone();
                ctx.current_task_ctx.set(tc);
                clone
            })
            .unwrap_or(None)
    }

    /// Returns the [`TaskId`] of the currently running task.
    ///
    /// # Panics
    ///
    /// This function panics if called from outside a task.For a version of this
    /// function that doesn't panic,
    /// see [`task::current::try_id()`](crate::task::current::try_id()).
    ///
    /// [task ID]: crate::task::Id
    pub fn id() -> TaskId {
        current_task_id().expect("Can't get a task id when not inside a task")
    }

    /// Returns the [`TaskId`] of the currently running task, or `None` if called outside
    /// of a task.
    ///
    /// This function is similar to  [`task::current::id()`](crate::task::current::id()), except
    /// that it returns `None` rather than panicking if called outside of a task
    /// context.
    ///
    /// [task ID]: crate::task::Id
    pub fn try_id() -> Option<TaskId> {
        current_task_id()
    }

    /// Returns the [`TaskContext`] of the currently running task.
    ///
    /// # Panics
    ///
    /// This function panics if called from outside a task. For a version of this
    /// function that doesn't panic,
    /// see [`task::current::try_id()`](crate::task::current::try_context()).
    ///
    /// [task ID]: crate::task::Id
    pub fn context() -> Arc<TaskContext> {
        current_task_ctx().expect("Can't get a task context when not inside a task")
    }

    /// Returns the [`TaskContext`] of the currently running task, or `None` if called outside
    /// of a task.
    ///
    /// This function is similar to  [`task::current::context()`](crate::task::current::context()), except
    /// that it returns `None` rather than panicking if called outside of a task
    /// context.
    ///
    /// [task ID]: crate::task::Id
    pub fn try_context() -> Option<Arc<TaskContext>> {
        current_task_ctx()
    }
}

impl std::fmt::Display for TaskId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl TaskId {
    /// Advance the atomic Id counter and return a new non zero Id
    pub(crate) fn next() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);

        loop {
            // Wraps arround on overflow, if a previous task hasn't been freed by then we have
            // problems
            let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            if let Some(id) = NonZeroU64::new(id) {
                return Self(id);
            }
        }
    }

    /// return this Id as a u64 number
    pub fn as_u64(&self) -> u64 {
        self.0.get()
    }
}

#[derive(Error, Debug)]
pub enum FfiError {
    #[error("error processing request: {0}")]
    Reqwest(#[from] reqwest::Error),
    #[error("error creating file '{:?}' : {}", .0.path, .0.error)]
    CreateFile(IoError),
    #[error("error writing to file '{:?}' : {}", .0.path, .0.error)]
    FileWrite(IoError),
    #[error("Error {}", .0.what())]
    Exception(crate::ffi::CxxException),
}

impl FfiError {
    /// Return a Some(&Cxxxception) is this error is form an exception
    pub fn as_exception(&self) -> Option<&crate::ffi::CxxException> {
        if let Self::Exception(external) = &self {
            Some(external)
        } else {
            None
        }
    }

    pub fn as_io(&self) -> Option<&IoError> {
        match &self {
            Self::CreateFile(err) | Self::FileWrite(err) => Some(err),
            _ => None,
        }
    }
}

#[derive(Error, Debug)]
pub struct IoError {
    pub path: OsString,
    pub error: io::Error,
}

impl IoError {
    pub fn path(&self) -> OsString {
        self.path.clone()
    }

    pub fn error(&self) -> String {
        self.error.to_string()
    }
}

impl Display for IoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.error)
    }
}

#[derive(Error, Debug)]
pub enum TaskSpawnError {
    #[error("task already awaited")]
    TaskAwaited,
    #[error("task spawn error: {0}")]
    Error(String),
}

impl TaskSpawnError {
    pub fn is_awaited(&self) -> bool {
        matches!(self, Self::TaskAwaited)
    }
}

#[derive(Error, Debug)]
pub enum TaskError {
    #[error("task canceled")]
    TaskCanceled,
    #[error("task join error: {0}")]
    JoinError(#[from] tokio::task::JoinError),
    #[error("{0}")]
    SpawnError(#[from] TaskSpawnError),
    #[error("{0}")]
    Error(#[from] FfiError),
}

impl TaskError {
    pub fn as_task_canceled(&self) -> Option<&TaskError> {
        if let Self::TaskCanceled = &self {
            Some(self)
        } else {
            None
        }
    }

    pub fn as_join_error(&self) -> Option<&tokio::task::JoinError> {
        if let Self::JoinError(je) = &self {
            Some(je)
        } else {
            None
        }
    }

    pub fn as_spawn_error(&self) -> Option<&TaskSpawnError> {
        if let Self::SpawnError(se) = &self {
            Some(se)
        } else {
            None
        }
    }

    pub fn as_error(&self) -> Option<&FfiError> {
        if let Self::Error(err) = &self {
            Some(err)
        } else {
            None
        }
    }
}

#[derive(Debug, Default, Copy, Clone)]
/// Represents a Task's progress event
///
/// * `progress`: the current progress
/// * `maximum`: the maximum progress
pub struct TaskProgress {
    progress: u64,
    maximum: u64,
}

impl TaskProgress {
    /// The current task progrss
    pub fn progress(&self) -> u64 {
        self.progress
    }
    /// The maximum progress value
    pub fn maximum(&self) -> u64 {
        self.maximum
    }
}

#[derive(Debug, Clone)]
/// Stored Task metadata
///
/// * `id`: Teh task Id
/// * `abort_handle`: a low level abord handle for the task
/// * `progress`: the progess event sender if the task has one
/// * `options`: options the task was spawned with (name, tags, ...)
/// * `context`: the task's context
pub struct TaskMetadata {
    id: TaskId,
    abort_handle: tokio::task::AbortHandle,
    #[allow(dead_code)]
    progress: Option<WatchReceiver<TaskProgress>>,
    options: TaskOptions,
    #[allow(dead_code)]
    context: Arc<TaskContext>,
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
        self.options.name.as_ref()
    }

    pub fn tags(&self) -> &HashSet<String> {
        &self.options.tags
    }
}

#[derive(Debug, Default, Clone)]
pub struct TaskSpawnOptions {
    name: Option<String>,
    tags: HashSet<String>,
    progress: Option<WatchReceiver<TaskProgress>>,
}

#[derive(Debug, Default, Clone)]
pub struct TaskOptions {
    pub name: Option<String>,
    pub tags: HashSet<String>,
}

impl TaskSpawnOptions {
    pub fn new() -> Self {
        TaskSpawnOptions::default()
    }

    pub fn named(name: String) -> Self {
        TaskSpawnOptions {
            name: Some(name),
            tags: HashSet::new(),
            progress: None,
        }
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

    pub fn add_tags<T, S>(&mut self, tags: T) -> &mut Self
    where
        T: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.tags
            .extend(tags.into_iter().map(|tag| tag.as_ref().to_owned()));
        self
    }

    pub fn set_tags<T, S>(&mut self, tags: T) -> &mut Self
    where
        T: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.tags.clear();
        self.tags
            .extend(tags.into_iter().map(|tag| tag.as_ref().to_owned()));
        self
    }

    pub fn to_options(&self) -> TaskOptions {
        TaskOptions {
            name: self.name.clone(),
            tags: self.tags.clone(),
        }
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = Some(name);
        self
    }

    pub fn with_tags<T, S>(mut self, tags: T) -> Self
    where
        T: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        self.tags
            .extend(tags.into_iter().map(|tag| tag.as_ref().to_owned()));
        self
    }

    pub fn with_progress(mut self, progress: WatchReceiver<TaskProgress>) -> Self {
        self.progress = Some(progress);
        self
    }
}

impl From<TaskSpawnOptions> for TaskOptions {
    fn from(value: TaskSpawnOptions) -> Self {
        value.to_options()
    }
}

#[derive(Debug, Default)]
pub struct TaskContext {
    progress: AtomicU64,
    progress_maximum: AtomicU64,
    progress_sender: Option<WatchSender<TaskProgress>>,
    cancel: AtomicBool,
    options: TaskOptions,
}

impl TaskContext {
    pub fn new(options: TaskOptions) -> Arc<Self> {
        Arc::new(TaskContext {
            progress: AtomicU64::new(0),
            progress_maximum: AtomicU64::new(0),
            progress_sender: None,
            cancel: AtomicBool::new(false),
            options,
        })
    }

    pub fn with_progress(options: TaskOptions, sender: WatchSender<TaskProgress>) -> Arc<Self> {
        Arc::new(TaskContext {
            progress: AtomicU64::new(0),
            progress_maximum: AtomicU64::new(0),
            progress_sender: Some(sender),
            cancel: AtomicBool::new(false),
            options,
        })
    }

    pub fn name(&self) -> Option<&String> {
        self.options.name.as_ref()
    }

    pub fn tags(&self) -> &HashSet<String> {
        &self.options.tags
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

    pub fn is_canceled(&self) -> bool {
        self.cancel.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
}

#[derive(Debug, Clone, Default)]
pub struct TaskListing(pub(crate) Arc<Mutex<HashMap<TaskId, Arc<TaskMetadata>>>>);

impl TaskListing {
    pub fn new() -> Self {
        TaskListing::default()
    }

    pub fn insert(&self, id: TaskId, metadata: Arc<TaskMetadata>) {
        self.0
            .lock()
            .expect("tasks listing lock poisoned")
            .insert(id, metadata);
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SubscriptionHandle(pub(crate) NonZeroU64);

impl Default for SubscriptionHandle {
    fn default() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        loop {
            let id = NEXT_ID.fetch_add(1, Ordering::SeqCst);
            if let Some(id) = NonZeroU64::new(id) {
                return SubscriptionHandle(id);
            }
        }
    }
}

impl SubscriptionHandle {
    fn new() -> Self {
        Self::default()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventType {
    Spawned,
    Finished,
}

pub struct EventSubscription {
    pub handle: SubscriptionHandle,
    pub ty: EventType,
    pub filter: Option<Box<dyn Fn(TaskId, &TaskOptions) -> bool + Send>>,
    pub callback: Box<dyn Fn(&TaskMetadata) + Send>,
}

impl std::fmt::Debug for EventSubscription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "EventSubscription({:?}, {:?}, filter: {})",
            &self.handle,
            &self.ty,
            self.filter.is_some()
        )
    }
}

#[derive(Debug, Default)]
pub struct SubscriptionListingInner {
    listing: HashMap<SubscriptionHandle, EventSubscription>,
    indexes: HashMap<EventType, HashSet<SubscriptionHandle>>,
}

impl SubscriptionListingInner {
    pub fn new() -> Self {
        SubscriptionListingInner::default()
    }

    pub fn insert(
        &mut self,
        handle: SubscriptionHandle,
        subscription: EventSubscription,
    ) -> Option<EventSubscription> {
        // remove the event from the indexes if it exists
        self.indexes.iter_mut().for_each(|(_ty, set)| {
            set.remove(&handle);
        });
        self.indexes
            .entry(subscription.ty)
            .or_default()
            .insert(handle);
        self.listing.insert(handle, subscription)
    }

    pub fn remove(&mut self, handle: &SubscriptionHandle) -> Option<EventSubscription> {
        self.indexes.iter_mut().for_each(|(_ty, set)| {
            set.remove(handle);
        });
        self.listing.remove(handle)
    }

    pub fn send_event(&self, event: EventType, meta: &TaskMetadata) {
        let handles = self.indexes.get(&event).cloned().unwrap_or_default();

        let subscriptions = self.listing.iter().filter_map(|(hdl, sub)| {
            if handles.contains(hdl) {
                Some(sub)
            } else {
                None
            }
        });
        for subscription in subscriptions {
            if let Some(filter) = &subscription.filter {
                if !filter(meta.id, &meta.options) {
                    continue;
                }
            }
            (subscription.callback)(meta);
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct SubscriptionListing {
    inner: Arc<Mutex<SubscriptionListingInner>>,
}

impl SubscriptionListing {
    pub fn new() -> Self {
        SubscriptionListing::default()
    }

    pub fn insert(&self, handle: SubscriptionHandle, subscription: EventSubscription) {
        self.inner
            .lock()
            .expect("subscription listing lock poisoned")
            .insert(handle, subscription);
    }

    pub fn remove(&self, handle: &SubscriptionHandle) {
        self.inner
            .lock()
            .expect("subscription listing lock poisoned")
            .remove(&handle);
    }

    pub fn send_event(&self, event: EventType, meta: &TaskMetadata) {
        self.inner
            .lock()
            .expect("subscription listing lock poisoned")
            .send_event(event, meta);
    }
}

#[derive(Debug, Clone)]
pub struct TaskEvent {
    pub ty: EventType,
    pub meta: Arc<TaskMetadata>,
}

#[derive(Clone)]
pub struct TaskManager {
    runtime: Arc<tokio::runtime::Runtime>,
    tasks: TaskListing,
    subscriptions: SubscriptionListing,
    event_sender: MSPCSender<TaskEvent>,
}

pub const EVENT_BUFFER_SIZE: Option<&'static str> = option_env!("TOKIO_TASK_EVENT_BUFFER_SIZE");

impl Default for TaskManager {
    fn default() -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .thread_name_fn(|| {
                    static ATOMIC_ID: AtomicUsize = AtomicUsize::new(1);
                    let id = ATOMIC_ID.fetch_add(1, Ordering::SeqCst);
                    format!("task-api-{}", id)
                })
                .enable_all()
                .build()
                .unwrap(),
        );
        let subscriptions = SubscriptionListing::new();
        let (tx, mut rx) = tokio::sync::mpsc::channel::<TaskEvent>(
            EVENT_BUFFER_SIZE
                .map(str::parse::<usize>)
                .transpose()
                .ok()
                .flatten()
                .unwrap_or(100),
        );
        let subs = subscriptions.clone();
        runtime.spawn(async move {
            while let Some(event) = rx.recv().await {
                subs.send_event(event.ty, &event.meta);
            }
        });
        TaskManager {
            runtime,
            tasks: TaskListing::new(),
            subscriptions,
            event_sender: tx,
        }
    }
}

impl TaskManager {
    /// Construct a new TaskManager
    pub fn new() -> Self {
        TaskManager::default()
    }

    /// Fetch the underlying tokio runtime
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

    /// spawn a future and record it as a [`Task`]
    pub fn spawn<T, F>(
        &self,
        fut: F,
        ctx: Arc<TaskContext>,
        options: Option<TaskSpawnOptions>,
    ) -> (TaskId, JoinHandle<Result<T, TaskError>>, Arc<TaskMetadata>)
    where
        T: Send + 'static,
        F: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        // make task
        let id = TaskId::next();
        let task_tracker = Arc::new(tokio::sync::Notify::new());
        let task = {
            let ctx = ctx.clone();
            let task_tracker = task_tracker.clone();
            // spawn a tracked task
            self.runtime.spawn(TaskFuture::new(
                async move {
                    let ret = fut.await;
                    task_tracker.notify_one();
                    ret
                },
                id,
                ctx.clone(),
            ))
        };
        // build task metadata
        let options = options.unwrap_or_default();
        let meta = Arc::new(TaskMetadata {
            id,
            abort_handle: task.abort_handle(),
            context: ctx,
            options: options.to_options(),
            progress: options.progress,
        });
        // store metadata on the task
        self.tasks.insert(meta.id, meta.clone());
        // send the task spawned event
        let _ = self.event_sender.blocking_send(TaskEvent {
            ty: EventType::Spawned,
            meta: meta.clone(),
        });
        {
            let meta = meta.clone();
            let tasks = self.tasks.clone();
            let event_sender = self.event_sender.clone();
            self.runtime.spawn(async move {
                // wait for task to finisk
                task_tracker.notified().await;
                // send task finished event
                let _ = event_sender
                    .send(TaskEvent {
                        ty: EventType::Finished,
                        meta,
                    })
                    .await;
                tasks.remove(id);
            });
        }
        (id, task, meta)
    }

    /// spawn a blocking function and record it as a [`Task`]
    pub fn spawn_blocking<T, F>(
        &self,
        fun: F,
        ctx: Arc<TaskContext>,
        options: Option<TaskSpawnOptions>,
    ) -> (TaskId, JoinHandle<Result<T, TaskError>>, Arc<TaskMetadata>)
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, TaskError> + Send + 'static,
    {
        // make task
        let id = TaskId::next();
        let task_tracker = Arc::new(tokio::sync::Notify::new());
        let task = {
            let ctx = ctx.clone();
            let task_tracker = task_tracker.clone();
            // spawn a tracked task
            self.runtime.spawn(async move {
                // spawn a blocking function in a special thread to avoid blocking the runtime
                let ret = tokio::task::spawn_blocking(move || {
                    // same gurad as a TaskFuture in a blocking context
                    let _guard = TaskGuard::enter(id, ctx.clone());
                    fun()
                })
                .await
                .map_err(Into::<TaskError>::into)
                .and_then(|ret| ret);
                task_tracker.notify_one();
                ret
            })
        };
        // build task metadata
        let options = options.unwrap_or_default();
        let meta = Arc::new(TaskMetadata {
            id,
            abort_handle: task.abort_handle(),
            context: ctx,
            options: options.to_options(),
            progress: options.progress,
        });
        // store metadata on the task
        self.tasks.insert(meta.id, meta.clone());
        // send the task spawned event
        let _ = self.event_sender.blocking_send(TaskEvent {
            ty: EventType::Spawned,
            meta: meta.clone(),
        });
        {
            let meta = meta.clone();
            let tasks = self.tasks.clone();
            let event_sender = self.event_sender.clone();
            self.runtime.spawn(async move {
                // wait for task to finisk
                task_tracker.notified().await;
                // send task finished event
                let _ = event_sender
                    .send(TaskEvent {
                        ty: EventType::Finished,
                        meta,
                    })
                    .await;
                tasks.remove(id);
            });
        }
        (id, task, meta)
    }

    pub fn new_task<T, F, R>(&self, f: F, options: Option<TaskSpawnOptions>) -> Box<Task<T>>
    where
        T: Send + 'static,
        F: FnOnce() -> R,
        R: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        Task::new(self, f, options)
    }

    pub fn new_blocking<T, F>(&self, f: F, options: Option<TaskSpawnOptions>) -> Box<Task<T>>
    where
        T: Send + 'static,
        F: FnOnce() -> Result<T, TaskError> + Send + 'static,
    {
        Task::blocking(self, f, options)
    }

    pub fn new_task_with_ctx<T, F, R>(
        &self,
        f: F,
        options: Option<TaskSpawnOptions>,
    ) -> Box<Task<T>>
    where
        T: Send + 'static,
        F: FnOnce(&TaskContext) -> R,
        R: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        Task::with_ctx(self, f, options)
    }

    pub fn new_blocking_with_ctx<T, F>(
        &self,
        f: F,
        options: Option<TaskSpawnOptions>,
    ) -> Box<Task<T>>
    where
        T: Send + 'static,
        F: FnOnce(&TaskContext) -> Result<T, TaskError> + Send + 'static,
    {
        Task::blocking_with_progress(self, f, options)
    }

    pub fn subscribe<Filter, Callback>(
        &self,
        event: EventType,
        filter: Option<Filter>,
        callback: Callback,
    ) -> SubscriptionHandle
    where
        Filter: Fn(TaskId, &TaskOptions) -> bool + Send + 'static,
        Callback: Fn(&TaskMetadata) + Send + 'static,
    {
        let handle = SubscriptionHandle::new();
        self.subscriptions.insert(
            handle,
            EventSubscription {
                handle,
                ty: event,
                filter: filter
                    .map(|f| -> Box<dyn Fn(TaskId, &TaskOptions) -> bool + Send> { Box::new(f) }),
                callback: Box::new(callback),
            },
        );
        handle
    }

    pub fn unsubscribe(&self, handle: &SubscriptionHandle) {
        self.subscriptions.remove(handle);
    }
}

/// Wrapper for a tokio Task handle that marks if it's already awaited elsewhere
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

/// Guard that ensure the thread_local current [`TaskId`] and [`TaskContext`] is correct
pub struct TaskGuard {
    parent_task_id: Option<TaskId>,
    parent_task_ctx: Option<Arc<TaskContext>>,
}

impl TaskGuard {
    fn enter(id: TaskId, ctx: Arc<TaskContext>) -> Self {
        TaskGuard {
            parent_task_id: current::set_current_task_id(Some(id)),
            parent_task_ctx: current::set_current_task_ctx(Some(ctx)),
        }
    }
}

impl Drop for TaskGuard {
    fn drop(&mut self) {
        current::set_current_task_id(self.parent_task_id);
        current::set_current_task_ctx(self.parent_task_ctx.take());
    }
}

/// Wrapper for a future that enteres a [`TaskIdGuard`] when polling
#[pin_project::pin_project]
pub struct TaskFuture<T, Fut>
where
    T: Send + 'static,
    Fut: Future<Output = T> + Send + 'static,
{
    #[pin]
    inner: Fut,
    id: TaskId,
    ctx: Arc<TaskContext>,
}

impl<T: Send + 'static, Fut: Future<Output = T> + Send + 'static> Future for TaskFuture<T, Fut> {
    type Output = T;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let _guard = TaskGuard::enter(self.id, self.ctx.clone());
        self.project().inner.poll(cx)
    }
}

impl<T: Send + 'static, Fut: Future<Output = T> + Send + 'static> TaskFuture<T, Fut> {
    fn new(fut: Fut, id: TaskId, ctx: Arc<TaskContext>) -> Self {
        TaskFuture {
            inner: fut,
            id,
            ctx,
        }
    }
}

pub struct Task<T: Send + 'static> {
    id: TaskId,
    manager: TaskManager,
    meta: Arc<TaskMetadata>,
    handle: Mutex<Option<TaskHandle<Result<T, TaskError>>>>,
    ctx: Arc<TaskContext>,
    progress: Option<WatchReceiver<TaskProgress>>,
}

impl<T: Send + 'static> Task<T> {
    pub fn new<F, R>(manager: &TaskManager, f: F, options: Option<TaskSpawnOptions>) -> Box<Self>
    where
        F: FnOnce() -> R,
        R: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        let tc = TaskContext::new(options.clone().unwrap_or_default().to_options());
        let ctx = tc.clone();
        let (id, task, meta) = manager.spawn(f(), tc, options);
        Box::new(Task {
            id,
            manager: manager.clone(),
            meta,
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx,
            progress: None,
        })
    }

    pub fn blocking<F>(manager: &TaskManager, f: F, options: Option<TaskSpawnOptions>) -> Box<Self>
    where
        F: FnOnce() -> Result<T, TaskError> + Send + 'static,
    {
        let tc = TaskContext::new(options.clone().unwrap_or_default().to_options());
        let ctx = tc.clone();
        let (id, task, meta) = manager.spawn_blocking(f, tc, options);
        Box::new(Task {
            id,
            manager: manager.clone(),
            meta,
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx,
            progress: None,
        })
    }

    pub fn with_ctx<F, R>(
        manager: &TaskManager,
        f: F,
        options: Option<TaskSpawnOptions>,
    ) -> Box<Self>
    where
        F: FnOnce(&TaskContext) -> R,
        R: Future<Output = Result<T, TaskError>> + Send + 'static,
    {
        let (ptx, prx) = tokio::sync::watch::channel(TaskProgress::default());
        let tc = TaskContext::with_progress(options.clone().unwrap_or_default().to_options(), ptx);
        let ctx = tc.clone();
        let (id, task, meta) = manager.spawn(
            f(&tc),
            tc.clone(),
            Some(options.unwrap_or_default().with_progress(prx.clone())),
        );
        Box::new(Task {
            id,
            manager: manager.clone(),
            meta,
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx,
            progress: Some(prx),
        })
    }

    pub fn blocking_with_progress<F>(
        manager: &TaskManager,
        f: F,
        options: Option<TaskSpawnOptions>,
    ) -> Box<Self>
    where
        F: FnOnce(&TaskContext) -> Result<T, TaskError> + Send + 'static,
    {
        let (ptx, prx) = tokio::sync::watch::channel(TaskProgress::default());
        let tc = TaskContext::with_progress(options.clone().unwrap_or_default().to_options(), ptx);
        let ctx = tc.clone();
        let (id, task, meta) = manager.spawn_blocking(
            move || f(&tc),
            ctx.clone(),
            Some(options.unwrap_or_default().with_progress(prx.clone())),
        );
        Box::new(Task {
            id,
            manager: manager.clone(),
            meta,
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx,
            progress: Some(prx),
        })
    }

    pub fn manager(&self) -> &TaskManager {
        &self.manager
    }

    pub fn then<T2, F>(
        &self,
        callback: F,
        options: Option<TaskSpawnOptions>,
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
        let tc = TaskContext::new(options.clone().unwrap_or_default().to_options());
        let ctx = tc.clone();
        let (id, task, meta) = self.manager.spawn(
            async move {
                let res = handle
                    .await
                    .map_err(Into::<TaskError>::into)
                    .and_then(|ret| ret);
                let id = current::id(); // recover id so it can be rest in blocking continuation
                tokio::task::spawn_blocking(move || {
                    let _guard = TaskGuard::enter(id, tc);
                    callback(res)
                })
                .await
                .map_err(Into::<TaskError>::into)
                .and_then(|ret| ret)
            },
            ctx.clone(),
            options,
        );
        Ok(Box::new(Task {
            id,
            manager: self.manager.clone(),
            meta,
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx,
            progress: None,
        }))
    }

    pub fn then_with_ctx<T2, F>(
        &self,
        callback: F,
        options: Option<TaskSpawnOptions>,
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
        let tc = TaskContext::with_progress(options.clone().unwrap_or_default().to_options(), ptx);
        let ctx = tc.clone();
        let (id, task, meta) = self.manager.spawn(
            async move {
                let res = handle
                    .await
                    .map_err(Into::<TaskError>::into)
                    .and_then(|ret| ret);
                let id = current::id(); // recover id so it can be rest in blocking continuation
                tokio::task::spawn_blocking(move || {
                    let _guard = TaskGuard::enter(id, tc.clone());
                    callback(res, &tc)
                })
                .await
                .map_err(Into::<TaskError>::into)
                .and_then(|ret| ret)
            },
            ctx.clone(),
            Some(options.unwrap_or_default().with_progress(prx.clone())),
        );
        Ok(Box::new(Task {
            id,
            manager: self.manager.clone(),
            meta,
            handle: Mutex::new(Some(TaskHandle::Owned(task))),
            ctx,
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
        self.ctx.cancel();
    }

    pub fn is_canceled(&self) -> bool {
        self.ctx.is_canceled()
    }

    pub fn on_progress<F>(&self, callback: F) -> Option<AbortHandle>
    where
        F: Fn(&TaskProgress) + Send + 'static,
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
