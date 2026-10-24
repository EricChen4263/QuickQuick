//! 探针：主线程与后台线程同时执行全部剪贴板读写路径，连续 N 轮不崩。
//!
//! 复现方法（会改写系统剪贴板内容，运行前请保存好剪贴板里的东西）：
//!
//! ```text
//! cd src-tauri
//! cargo run --example pasteboard_serial_probe -- --iterations 700 --workers 4        # 串行通道，应全程不崩
//! cargo run --example pasteboard_serial_probe -- --iterations 700 --workers 4 --no-dispatch  # 变异：去掉主线程派发，应崩溃
//! ```
//!
//! 每轮覆盖的路径：`changeCount` 标量读、变化时的全量读（文本 + HTML + 图片 + 类型标记）、
//! 文本写回（含自产标记）、图片写回，以及 `MacOsPasteBackend` 的 `changeCount`/读文本。
//! 工作线程与主线程**同时**跑这些路径：真正的 `main()` 线程既是串行通道的执行线程
//! （泵任务队列），也在泵的间隙自己直接访问剪贴板。
//!
//! `--no-dispatch` 把执行器换成「就地执行」：任务不再投递到主线程，每个线程各自访问
//! `NSPasteboard`——这是被探针证明会 SIGSEGV 的并发形态，用来证明探针有可失败性。

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("该探针只适用于 macOS");
}

#[cfg(target_os = "macos")]
fn main() {
    probe::run();
}

#[cfg(target_os = "macos")]
mod probe {
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::mpsc::{self, RecvTimeoutError};
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use quickquick_lib::clipboard::{CapturedItem, ClipboardBackend};
    use quickquick_lib::macos_paste::MacOsPasteBackend;
    use quickquick_lib::paste::PasteBackend;
    use quickquick_lib::pasteboard_meta::LastExternalSourceApp;
    use quickquick_lib::pasteboard_serial::{
        install_global, is_main_thread, MainThreadExecutor, MainThreadPasteboardBackend, SerialJob,
        SerialPasteboard,
    };

    /// 真主线程泵：任务进队列，由 `main()` 线程取出执行。
    struct MainPumpExecutor {
        sender: Mutex<mpsc::Sender<SerialJob>>,
    }

    impl MainThreadExecutor for MainPumpExecutor {
        fn submit(&self, job: SerialJob) -> Result<(), String> {
            self.sender
                .lock()
                .map_err(|e| e.to_string())?
                .send(job)
                .map_err(|e| e.to_string())
        }

        fn on_executor_thread(&self) -> bool {
            is_main_thread()
        }
    }

    /// 变异执行器：去掉主线程派发，任务在调用线程上就地执行。
    struct InlineExecutor;

    impl MainThreadExecutor for InlineExecutor {
        fn submit(&self, job: SerialJob) -> Result<(), String> {
            job();
            Ok(())
        }

        fn on_executor_thread(&self) -> bool {
            true
        }
    }

    struct Args {
        iterations: u64,
        workers: usize,
        no_dispatch: bool,
    }

    fn parse_args() -> Args {
        let mut args = Args {
            iterations: 700,
            workers: 4,
            no_dispatch: false,
        };
        let mut it = std::env::args().skip(1);
        while let Some(flag) = it.next() {
            match flag.as_str() {
                "--iterations" => {
                    args.iterations = it
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(args.iterations)
                }
                "--workers" => {
                    args.workers = it
                        .next()
                        .and_then(|v| v.parse().ok())
                        .unwrap_or(args.workers)
                }
                "--no-dispatch" => args.no_dispatch = true,
                other => eprintln!("忽略未知参数：{other}"),
            }
        }
        args
    }

    /// 一轮完整的后台读写路径；返回本轮真正执行的访问次数。
    fn one_cycle(
        reader: &MainThreadPasteboardBackend,
        paster: &mut MacOsPasteBackend,
        round: u64,
        counters: &Counters,
    ) {
        let seen = reader.change_count();
        counters.count_scalar.fetch_add(1, Ordering::Relaxed);
        if reader.read_observed(seen).is_some() {
            counters.full_reads.fetch_add(1, Ordering::Relaxed);
        }
        let _ = paster.change_count();
        let _ = paster.current_text();
        counters.scalar_paste.fetch_add(2, Ordering::Relaxed);

        // 写路径：文本（偶数轮带 HTML）与小图交替，保证读线程总有新内容可读
        if round % 3 == 2 {
            let rgba = vec![200u8; 32 * 32 * 4];
            if paster.write_image(32, 32, &rgba).is_ok() {
                counters.writes.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            let item = CapturedItem {
                text: format!("probe-{round}"),
                html: round
                    .is_multiple_of(2)
                    .then(|| format!("<b>probe-{round}</b>")),
            };
            paster.write_with_marker(&item);
            counters.writes.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[derive(Default)]
    struct Counters {
        count_scalar: AtomicU64,
        full_reads: AtomicU64,
        scalar_paste: AtomicU64,
        writes: AtomicU64,
        main_direct: AtomicU64,
    }

    pub fn run() {
        let args = parse_args();
        let (sender, receiver) = mpsc::channel::<SerialJob>();
        let executor: Arc<dyn MainThreadExecutor> = if args.no_dispatch {
            Arc::new(InlineExecutor)
        } else {
            Arc::new(MainPumpExecutor {
                sender: Mutex::new(sender),
            })
        };
        let serial = Arc::new(SerialPasteboard::new(executor));
        assert!(install_global(Arc::clone(&serial)), "全局通道应是首次安装");

        let tracker = Arc::new(LastExternalSourceApp::new());
        let counters = Arc::new(Counters::default());
        let done = Arc::new(AtomicBool::new(false));
        let started = Instant::now();

        let workers: Vec<_> = (0..args.workers)
            .map(|_| {
                let serial = Arc::clone(&serial);
                let tracker = Arc::clone(&tracker);
                let counters = Arc::clone(&counters);
                let iterations = args.iterations;
                std::thread::spawn(move || {
                    let reader = MainThreadPasteboardBackend::new(serial, tracker);
                    let mut paster = MacOsPasteBackend::new().expect("MacOsPasteBackend 应可创建");
                    for round in 0..iterations {
                        one_cycle(&reader, &mut paster, round, &counters);
                    }
                })
            })
            .collect();

        // 主线程：泵队列 + 在间隙里自己直接访问剪贴板（经通道，主线程上就地执行）
        let main_reader =
            MainThreadPasteboardBackend::new(Arc::clone(&serial), Arc::clone(&tracker));
        let main_paster = MacOsPasteBackend::new().expect("主线程 MacOsPasteBackend");
        let waiter = {
            let done = Arc::clone(&done);
            std::thread::spawn(move || {
                for worker in workers {
                    worker.join().expect("工作线程不应 panic");
                }
                done.store(true, Ordering::SeqCst);
            })
        };
        let mut main_round = 0u64;
        while !done.load(Ordering::SeqCst) {
            match receiver.recv_timeout(Duration::from_millis(1)) {
                Ok(job) => job(),
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
            }
            let seen = main_reader.change_count();
            let _ = main_reader.read_observed(seen);
            let _ = main_paster.current_text();
            main_round += 1;
            counters.main_direct.fetch_add(1, Ordering::Relaxed);
        }
        let _ = main_round;
        waiter.join().expect("等待线程不应 panic");

        let total_cycles = args.iterations * args.workers as u64;
        println!(
            "PROBE_OK mode={} workers={} iterations_per_worker={} total_worker_cycles={} \
             worker_full_reads={} worker_writes={} main_thread_direct_cycles={} elapsed_ms={}",
            if args.no_dispatch {
                "NO-DISPATCH(变异)"
            } else {
                "serial"
            },
            args.workers,
            args.iterations,
            total_cycles,
            counters.full_reads.load(Ordering::Relaxed),
            counters.writes.load(Ordering::Relaxed),
            counters.main_direct.load(Ordering::Relaxed),
            started.elapsed().as_millis()
        );
    }
}
