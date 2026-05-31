//! 在途请求取消框架（V2-F1-S03 A07）。
//!
//! 解决"连续选中即译"场景：用户快速连续选中文本时，旧的翻译请求应被
//! 作废，只有最新一次请求的结果才被采纳。
//!
//! 实现思路：单调递增的 generation 计数器。每次发起新请求调用 `begin()`
//! 拿到新 generation，同时使所有旧 generation 失效。收到响应时用
//! `is_current(gen)` 判断是否仍为最新——若不是，直接丢弃结果。

use std::sync::atomic::{AtomicU64, Ordering};

/// 在途请求追踪器。
///
/// 内部持一个单调递增的 generation 计数器。
/// `begin()` 自增并返回新 generation；`is_current(gen)` 检查该 generation
/// 是否仍为最新。线程安全（`AtomicU64`）。
pub struct InflightTracker {
    current: AtomicU64,
}

impl InflightTracker {
    /// 创建新追踪器，初始 generation 为 0（未发起任何请求）。
    pub fn new() -> Self {
        Self {
            current: AtomicU64::new(0),
        }
    }

    /// 发起新请求：generation 自增，返回本次请求的 generation 编号。
    ///
    /// 调用后，所有持有旧 generation 的在途请求均视为已作废。
    pub fn begin(&self) -> u64 {
        self.current.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// 判断给定 generation 是否仍为当前最新请求。
    ///
    /// 仅当 `gen == current` 时返回 true；旧请求收到响应后应先调用此方法，
    /// false 则直接丢弃响应，避免过时结果覆盖更新的译文。
    pub fn is_current(&self, gen: u64) -> bool {
        self.current.load(Ordering::SeqCst) == gen
    }
}

impl Default for InflightTracker {
    fn default() -> Self {
        Self::new()
    }
}
