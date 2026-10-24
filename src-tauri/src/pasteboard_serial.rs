//! 剪贴板访问的单线程串行通道
//!
//! # 为什么需要它
//!
//! macOS 的 `NSPasteboard` **不是线程安全的**：同一进程内两个线程同时访问
//! （`types()`、`changeCount`、arboard 的正文读写都算）会 SIGSEGV。实测对照：
//! 并行 3/3 次崩溃，串行到同一线程 3/3 次不崩。崩的是整个进程，不是丢一次捕获。
//!
//! 本模块把进程内**全部** `NSPasteboard` 访问收敛到一个线程（Tauri 主线程）：
//! - 轮询线程用 [`MainThreadPasteboardBackend`] 读：`changeCount` 是一个标量，
//!   每轮在主线程只付一次标量读取；只有它变了才在主线程做**一次**全量读
//!   （文本 + HTML + 图片 + 类型标记），快照交回后台线程去哈希、入库；
//! - 写回路径（复制 / 粘贴）经 [`run_global`] 进入同一通道；
//! - 已经在串行线程上的调用方就地执行，不排队、不自等死锁。
//!
//! # 线程模型与锁序
//!
//! 通道的执行线程会同步执行投递来的任务，所以**调用方在等待通道期间不得持有主线程
//! 也可能争用的锁**（`AppDb` 连接锁、忽略规则读写锁）：否则会出现「调用方持锁等主线程、
//! 主线程等锁」的互等，只能靠超时解开。轮询循环因此先在无锁状态下取得
//! [`crate::clipboard::ObservedClipboard`]，再取锁入库。
//!
//! # 超时语义
//!
//! 等待通道有上限（见 `*_TIMEOUT` 常量）。超时只表示调用方放弃等待，已投递的任务
//! 仍会在执行线程上跑完，其结果被丢弃。读路径对此无害（下一轮重试）；写路径若超时，
//! 调用方会收到错误，但写入可能稍后才真正发生——这是为避免无限期挂住而接受的代价。

use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{mpsc, Arc, OnceLock};
use std::time::Duration;

/// 投递到串行线程的任务。
pub type SerialJob = Box<dyn FnOnce() + Send + 'static>;

/// 读取 `changeCount` 标量的等待上限：正常是微秒级，留两个数量级余量。
pub const SCALAR_READ_TIMEOUT: Duration = Duration::from_millis(300);

/// 全量读取（文本 + HTML + 图片解码）的等待上限：大图解码在主线程上要几百毫秒。
pub const FULL_READ_TIMEOUT: Duration = Duration::from_secs(3);

/// 写回剪贴板的等待上限：图片写入要先编码为 TIFF。
pub const WRITE_TIMEOUT: Duration = Duration::from_secs(3);

/// 唯一允许访问剪贴板的线程的执行器。
///
/// 抽成 trait 的目的：生产用 Tauri 主线程；探针与单测注入「真主线程泵」或「专用线程」，
/// 用同一份通道逻辑验证串行化不变量。
pub trait MainThreadExecutor: Send + Sync {
    /// 把任务交给执行线程；未能投递（事件循环已退出等）返回原因。
    ///
    /// # Errors
    /// 任务没有被接受时返回错误描述。
    fn submit(&self, job: SerialJob) -> Result<(), String>;

    /// 当前线程是否就是执行线程（是则调用方就地执行，避免线程等待自己）。
    fn on_executor_thread(&self) -> bool;
}

/// 通过通道执行任务失败的原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SerialError {
    /// 执行器拒绝了任务
    Dispatch(String),
    /// 等待执行线程完成超过上限
    Timeout(Duration),
    /// 任务在执行线程上 panic（已被拦住，没有波及执行线程）
    Panicked,
}

impl std::fmt::Display for SerialError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Dispatch(reason) => write!(f, "投递到串行线程失败：{reason}"),
            Self::Timeout(limit) => write!(f, "等待串行线程超时（{}ms）", limit.as_millis()),
            Self::Panicked => write!(f, "串行线程上的剪贴板任务 panic"),
        }
    }
}

impl std::error::Error for SerialError {}

/// 单线程串行通道：把任务交给执行器并带超时地等结果。
pub struct SerialPasteboard {
    executor: Arc<dyn MainThreadExecutor>,
}

impl SerialPasteboard {
    /// 用给定执行器构造通道。
    #[must_use]
    pub fn new(executor: Arc<dyn MainThreadExecutor>) -> Self {
        Self { executor }
    }

    /// 当前线程是否就是串行线程。
    #[must_use]
    pub fn on_serial_thread(&self) -> bool {
        self.executor.on_executor_thread()
    }

    /// 在串行线程上执行 `f` 并返回结果。
    ///
    /// 已在串行线程上时就地执行；否则投递并最多等待 `timeout`。任务 panic 会被拦下并
    /// 以 [`SerialError::Panicked`] 返回：在 Tauri 主线程上 panic 会穿出事件循环、
    /// 直接终止进程，而任务里有图片解码这类第三方代码，不能把它的 panic 当成自己的。
    ///
    /// # Errors
    /// 投递失败、超时或任务 panic 时返回对应的 [`SerialError`]。
    pub fn run<T, F>(&self, timeout: Duration, f: F) -> Result<T, SerialError>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        if self.executor.on_executor_thread() {
            return catch_unwind(AssertUnwindSafe(f)).map_err(|_| SerialError::Panicked);
        }

        let (tx, rx) = mpsc::channel::<std::thread::Result<T>>();
        let job: SerialJob = Box::new(move || {
            // 接收端可能已超时放弃，发送失败不值得记录
            let _ = tx.send(catch_unwind(AssertUnwindSafe(f)));
        });
        self.executor.submit(job).map_err(SerialError::Dispatch)?;

        match rx.recv_timeout(timeout) {
            Ok(Ok(value)) => Ok(value),
            Ok(Err(_)) => Err(SerialError::Panicked),
            Err(_) => Err(SerialError::Timeout(timeout)),
        }
    }
}

/// 生产执行器：Tauri 主线程（AppKit run loop 所在线程）。
pub struct TauriMainThreadExecutor {
    app: tauri::AppHandle,
}

impl TauriMainThreadExecutor {
    /// 用应用句柄构造。
    #[must_use]
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl MainThreadExecutor for TauriMainThreadExecutor {
    fn submit(&self, job: SerialJob) -> Result<(), String> {
        self.app
            .run_on_main_thread(job)
            .map_err(|e| format!("run_on_main_thread 失败：{e}"))
    }

    fn on_executor_thread(&self) -> bool {
        is_main_thread()
    }
}

/// 当前线程是否就是 macOS 主线程。
///
/// 非 macOS 平台没有「必须主线程」的约束，也不会构造 Tauri 执行器，返回 `false`。
#[cfg(target_os = "macos")]
#[must_use]
pub fn is_main_thread() -> bool {
    objc2::MainThreadMarker::new().is_some()
}

/// 当前线程是否就是 macOS 主线程（非 macOS 恒为 `false`）。
#[cfg(not(target_os = "macos"))]
#[must_use]
pub fn is_main_thread() -> bool {
    false
}

/// 进程级串行通道。
///
/// 为什么需要全局：复制 / 粘贴的写回入口（`ipc/system.rs`）每次调用现场创建
/// `arboard::Clipboard` 再调 `macos_paste::write_*_to_clipboard`，没有地方携带通道句柄。
/// 全局只在启动时安装一次（[`install_global`]），之后只读；未安装时（单测、无头环境）
/// 一切就地执行，等价于调用方自己保证单线程。
static GLOBAL: OnceLock<Arc<SerialPasteboard>> = OnceLock::new();

/// 安装进程级串行通道；已安装过则返回 `false` 且不覆盖。
pub fn install_global(serial: Arc<SerialPasteboard>) -> bool {
    GLOBAL.set(serial).is_ok()
}

/// 当前线程是否已处在串行线程上（未安装全局通道时视为是）。
#[must_use]
pub fn is_serial_thread() -> bool {
    GLOBAL.get().is_none_or(|serial| serial.on_serial_thread())
}

/// 经进程级通道在串行线程上执行 `f`；未安装全局通道时就地执行。
///
/// # Errors
/// 同 [`SerialPasteboard::run`]。
pub fn run_global<T, F>(timeout: Duration, f: F) -> Result<T, SerialError>
where
    T: Send + 'static,
    F: FnOnce() -> T + Send + 'static,
{
    match GLOBAL.get() {
        Some(serial) => serial.run(timeout, f),
        None => catch_unwind(AssertUnwindSafe(f)).map_err(|_| SerialError::Panicked),
    }
}

/// 读 `NSPasteboard.changeCount`。**必须在串行线程上调用。**
///
/// `changeCount` 是 `NSInteger`，实践中单调递增且非负；负值防御性收敛为 0。
#[cfg(target_os = "macos")]
#[must_use]
pub fn read_change_count() -> u64 {
    objc2_app_kit::NSPasteboard::generalPasteboard()
        .changeCount()
        .max(0) as u64
}

#[cfg(target_os = "macos")]
pub use backend::MainThreadPasteboardBackend;

/// macOS 生产剪贴板后端：全部 `NSPasteboard` 读取都在串行线程上完成。
#[cfg(target_os = "macos")]
mod backend {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{read_change_count, SerialPasteboard, FULL_READ_TIMEOUT, SCALAR_READ_TIMEOUT};
    use crate::clipboard::{ClipboardBackend, ClipboardSnapshot, RawImageData, SourceApp};
    use crate::pasteboard_meta::{log_read_failure, read_pasteboard_meta, LastExternalSourceApp};

    /// 一次全量读的结果：快照、读取瞬间的 `changeCount`、来源应用身份。
    struct FullRead {
        count: u64,
        snapshot: ClipboardSnapshot,
        source_app: Option<SourceApp>,
    }

    /// 全量读：计数、文本、HTML、图片、类型标记、来源应用，全部在同一任务内完成。
    ///
    /// 计数在**读取之前**取：读取期间若别的进程又改了剪贴板，计数偏小，下一轮会再读一次，
    /// 重复内容由库的去重（Bump）吸收；若在读取之后取，则会把这次中途变化吞掉。
    fn read_full(tracker: &LastExternalSourceApp) -> Result<FullRead, String> {
        let count = read_change_count();
        let mut clipboard =
            arboard::Clipboard::new().map_err(|e| format!("剪贴板句柄创建失败：{e}"))?;

        // arboard 读不到对应类型时返回 Err（ContentNotAvailable 等），属常态，降级为 None
        let text = clipboard.get_text().ok();
        // get() 是一次性 builder，故 html 与 text/image 分别取
        let html = clipboard.get().html().ok();
        let image = clipboard.get_image().ok().map(|img| RawImageData {
            width: img.width,
            height: img.height,
            bytes: img.bytes.into_owned(),
        });

        // 敏感 / 自产标记 / 来源应用来自同一次读取，描述的是同一份内容
        let meta = read_pasteboard_meta(tracker);
        let source_app = meta.source_app.clone();
        Ok(FullRead {
            count,
            snapshot: ClipboardSnapshot {
                text,
                html,
                image,
                has_self_marker: meta.has_self_marker,
                is_concealed: meta.is_concealed,
                source_app: source_app.as_ref().and_then(SourceApp::primary_identifier),
            },
            source_app,
        })
    }

    /// macOS 生产后端：`changeCount` 标量检测 + 变化时一次全量读，全部经串行通道。
    ///
    /// 与旧的内容哈希轮询相比：主线程每轮只付一次标量读取，不再每 500ms 把文本、HTML、
    /// 图片 RGBA 全读一遍再哈希。去重语义不变：同内容再次复制会走到入库，
    /// 由 `db::ingest` 命中同 hash 后置顶（Bump）。
    pub struct MainThreadPasteboardBackend {
        serial: Arc<SerialPasteboard>,
        tracker: Arc<LastExternalSourceApp>,
        /// 最近一次成功读到的计数；串行线程忙或读取失败时回放它，保证「不误触发」
        last_count: AtomicU64,
        /// 最近一次全量读采集的来源应用身份，供 `source_app_identity` 回放
        last_source_app: Mutex<Option<SourceApp>>,
    }

    impl MainThreadPasteboardBackend {
        /// 用串行通道与前台应用记录构造。
        #[must_use]
        pub fn new(serial: Arc<SerialPasteboard>, tracker: Arc<LastExternalSourceApp>) -> Self {
            Self {
                serial,
                tracker,
                last_count: AtomicU64::new(0),
                last_source_app: Mutex::new(None),
            }
        }

        fn remember_source_app(&self, source_app: Option<SourceApp>) {
            *self
                .last_source_app
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = source_app;
        }
    }

    impl ClipboardBackend for MainThreadPasteboardBackend {
        fn change_count(&self) -> u64 {
            match self.serial.run(SCALAR_READ_TIMEOUT, read_change_count) {
                Ok(count) => {
                    self.last_count.store(count, Ordering::Relaxed);
                    count
                }
                Err(e) => {
                    log_read_failure("macOS changeCount 串行读取", &e.to_string());
                    self.last_count.load(Ordering::Relaxed)
                }
            }
        }

        fn read(&self) -> ClipboardSnapshot {
            self.read_observed(self.last_count.load(Ordering::Relaxed))
                .map_or_else(ClipboardSnapshot::empty, |(_, snapshot)| snapshot)
        }

        fn read_observed(&self, _seen_count: u64) -> Option<(u64, ClipboardSnapshot)> {
            let tracker = Arc::clone(&self.tracker);
            let outcome = self
                .serial
                .run(FULL_READ_TIMEOUT, move || read_full(&tracker));
            match outcome {
                Ok(Ok(full)) => {
                    self.last_count.store(full.count, Ordering::Relaxed);
                    self.remember_source_app(full.source_app);
                    Some((full.count, full.snapshot))
                }
                Ok(Err(reason)) => {
                    log_read_failure("macOS 全量读取", &reason);
                    self.remember_source_app(None);
                    None
                }
                Err(e) => {
                    log_read_failure("macOS 全量读取（串行通道）", &e.to_string());
                    self.remember_source_app(None);
                    None
                }
            }
        }

        fn source_app_identity(&self) -> Option<SourceApp> {
            self.last_source_app
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clone()
        }
    }
}

#[cfg(test)]
mod tests {
    //! 通道逻辑的单测只用「专用线程」执行器，**不触碰真实 `NSPasteboard`**：
    //! 真实读写的并发安全由 `examples/pasteboard_serial_probe.rs` 验证。

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use std::thread::ThreadId;

    use super::*;

    /// 单一专用线程顺序执行全部任务，模拟「主线程」。
    struct DedicatedThreadExecutor {
        sender: Mutex<mpsc::Sender<SerialJob>>,
        thread_id: ThreadId,
        submitted: AtomicUsize,
    }

    impl DedicatedThreadExecutor {
        fn spawn() -> Arc<Self> {
            let (sender, receiver) = mpsc::channel::<SerialJob>();
            let (id_tx, id_rx) = mpsc::channel();
            std::thread::spawn(move || {
                id_tx
                    .send(std::thread::current().id())
                    .expect("回传线程 id");
                for job in receiver {
                    job();
                }
            });
            Arc::new(Self {
                sender: Mutex::new(sender),
                thread_id: id_rx.recv().expect("取得执行线程 id"),
                submitted: AtomicUsize::new(0),
            })
        }
    }

    impl MainThreadExecutor for DedicatedThreadExecutor {
        fn submit(&self, job: SerialJob) -> Result<(), String> {
            self.submitted.fetch_add(1, Ordering::SeqCst);
            self.sender
                .lock()
                .expect("sender 锁")
                .send(job)
                .map_err(|e| e.to_string())
        }

        fn on_executor_thread(&self) -> bool {
            std::thread::current().id() == self.thread_id
        }
    }

    /// 永远拒绝投递的执行器。
    struct RejectingExecutor;

    impl MainThreadExecutor for RejectingExecutor {
        fn submit(&self, _job: SerialJob) -> Result<(), String> {
            Err("事件循环已退出".to_owned())
        }

        fn on_executor_thread(&self) -> bool {
            false
        }
    }

    /// 接受任务但永不执行的执行器：用来制造超时。
    struct BlackHoleExecutor;

    impl MainThreadExecutor for BlackHoleExecutor {
        fn submit(&self, _job: SerialJob) -> Result<(), String> {
            Ok(())
        }

        fn on_executor_thread(&self) -> bool {
            false
        }
    }

    #[test]
    fn run_from_other_thread_executes_on_executor_thread_and_returns_value() {
        // Arrange
        let executor = DedicatedThreadExecutor::spawn();
        let executor_thread = executor.thread_id;
        let serial = SerialPasteboard::new(executor.clone());

        // Act
        let (value, ran_on) = serial
            .run(Duration::from_secs(2), || {
                (7_u32, std::thread::current().id())
            })
            .expect("任务应执行成功");

        // Assert：结果原样返回，且任务确实在执行线程而不是调用线程上跑
        assert_eq!(value, 7);
        assert_eq!(ran_on, executor_thread, "任务必须在执行线程上运行");
        assert_ne!(ran_on, std::thread::current().id());
        assert_eq!(executor.submitted.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn run_on_executor_thread_executes_inline_without_submitting() {
        // Arrange：外层任务本身就在执行线程上，再嵌套一次 run
        let executor = DedicatedThreadExecutor::spawn();
        let serial = Arc::new(SerialPasteboard::new(executor.clone()));
        let inner_serial = Arc::clone(&serial);

        // Act
        let nested = serial
            .run(Duration::from_secs(2), move || {
                inner_serial.run(Duration::from_millis(50), || 42_u32)
            })
            .expect("外层应成功");

        // Assert：嵌套调用就地执行（若排队会自等死锁并超时），且只投递了外层这一次
        assert_eq!(nested, Ok(42));
        assert_eq!(executor.submitted.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn run_reports_dispatch_error_when_executor_rejects_job() {
        // Arrange
        let serial = SerialPasteboard::new(Arc::new(RejectingExecutor));

        // Act
        let outcome = serial.run(Duration::from_millis(50), || 1_u32);

        // Assert
        assert_eq!(
            outcome,
            Err(SerialError::Dispatch("事件循环已退出".to_owned()))
        );
    }

    #[test]
    fn run_times_out_when_executor_never_runs_job() {
        // Arrange
        let serial = SerialPasteboard::new(Arc::new(BlackHoleExecutor));
        let limit = Duration::from_millis(30);

        // Act
        let outcome = serial.run(limit, || 1_u32);

        // Assert：返回超时而不是无限挂住
        assert_eq!(outcome, Err(SerialError::Timeout(limit)));
    }

    #[test]
    fn panicking_job_is_contained_and_executor_keeps_working() {
        // Arrange
        let serial = SerialPasteboard::new(DedicatedThreadExecutor::spawn());

        // Act
        let panicked = serial.run(Duration::from_secs(2), || -> u32 {
            panic!("模拟图片解码 panic")
        });
        let after = serial.run(Duration::from_secs(2), || 5_u32);

        // Assert：panic 被转成错误，执行线程没有被它带走
        assert_eq!(panicked, Err(SerialError::Panicked));
        assert_eq!(after, Ok(5), "panic 之后执行线程应继续可用");
    }

    #[test]
    fn concurrent_callers_never_overlap_on_executor_thread() {
        // Arrange：8 个调用线程各投递 100 个任务，任务内记录并发度与线程 id
        let executor = DedicatedThreadExecutor::spawn();
        let executor_thread = executor.thread_id;
        let serial = Arc::new(SerialPasteboard::new(executor));
        let in_flight = Arc::new(AtomicUsize::new(0));
        let max_in_flight = Arc::new(AtomicUsize::new(0));
        let foreign_thread_runs = Arc::new(AtomicUsize::new(0));

        // Act
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let serial = Arc::clone(&serial);
                let in_flight = Arc::clone(&in_flight);
                let max_in_flight = Arc::clone(&max_in_flight);
                let foreign = Arc::clone(&foreign_thread_runs);
                std::thread::spawn(move || {
                    for _ in 0..100 {
                        let (in_flight, max_in_flight, foreign) = (
                            Arc::clone(&in_flight),
                            Arc::clone(&max_in_flight),
                            Arc::clone(&foreign),
                        );
                        serial
                            .run(Duration::from_secs(5), move || {
                                let now = in_flight.fetch_add(1, Ordering::SeqCst) + 1;
                                max_in_flight.fetch_max(now, Ordering::SeqCst);
                                if std::thread::current().id() != executor_thread {
                                    foreign.fetch_add(1, Ordering::SeqCst);
                                }
                                std::thread::yield_now();
                                in_flight.fetch_sub(1, Ordering::SeqCst);
                            })
                            .expect("任务应执行成功");
                    }
                })
            })
            .collect();
        for handle in handles {
            handle.join().expect("调用线程不应 panic");
        }

        // Assert：800 个任务从不并发、从不落在执行线程之外
        assert_eq!(max_in_flight.load(Ordering::SeqCst), 1, "任务不得并发执行");
        assert_eq!(foreign_thread_runs.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn global_helpers_run_inline_when_not_installed() {
        // 单测进程内不安装全局通道（OnceLock 一旦设置无法撤销，安装路径由探针覆盖）
        assert!(GLOBAL.get().is_none(), "单测进程不应安装全局通道");

        // Act
        let caller = std::thread::current().id();
        let outcome = run_global(Duration::from_millis(50), move || {
            std::thread::current().id()
        });

        // Assert：未安装时就地执行，且视作已在串行线程
        assert_eq!(outcome, Ok(caller));
        assert!(is_serial_thread());
    }
}
