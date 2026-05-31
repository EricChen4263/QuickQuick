//! 错误降级与同源退避重试策略（V2-F1-S03 A04）。
//!
//! 设计约束：
//! - 瞬时错误（Network / RateLimit / ServerError）在**同一 provider** 内退避重试。
//! - 永久错误（Auth / Quota / Unsupported / TooLong / ParseError）立即返回，不重试。
//! - **框架绝不自动切换 provider**——跨源切换由调用方显式发起。

use super::TranslateError;

/// 判断错误是否为瞬时可重试。
///
/// - 瞬时（true）：`Network` / `RateLimit` / `ServerError`——短暂抖动，退避后可恢复。
/// - 永久（false）：`Auth` / `Quota` / `Unsupported` / `TooLong` / `ParseError`——需人工干预或换 provider。
pub fn is_transient(err: &TranslateError) -> bool {
    matches!(
        err,
        TranslateError::Network(_) | TranslateError::RateLimit(_) | TranslateError::ServerError(_)
    )
}

/// 同源退避重试执行器。
///
/// 对 `op` 最多调用 `max_attempts` 次：
/// - 每次失败且为瞬时错误，用 `sleep_fn` 等待退避后继续；
/// - 永久错误立即返回，不消耗剩余重试次数；
/// - 耗尽重试次数后，返回最后一次错误。
///
/// `sleep_fn` 接收退避毫秒数，由调用方决定实际等待行为：
/// - 生产：`|ms| std::thread::sleep(std::time::Duration::from_millis(ms))`
/// - 测试：记录被调用值的闭包（可断言退避序列），不引入真实阻塞
///
/// **不含任何跨 provider 切换逻辑**——保证同源语义。
pub fn retry_with_backoff<T, F, S>(
    max_attempts: u32,
    mut op: F,
    sleep_fn: S,
) -> Result<T, TranslateError>
where
    F: FnMut() -> Result<T, TranslateError>,
    S: Fn(u64),
{
    let mut last_err = TranslateError::Network("未执行任何尝试".to_string());

    for attempt in 0..max_attempts {
        match op() {
            Ok(val) => return Ok(val),
            Err(err) if !is_transient(&err) => return Err(err),
            Err(err) => {
                sleep_fn(next_backoff_ms(attempt));
                last_err = err;
            }
        }
    }

    Err(last_err)
}

/// 计算第 `attempt` 次重试的退避毫秒数（指数退避，上限 8000ms）。
///
/// attempt=0 → 500ms，attempt=1 → 1000ms，attempt=2 → 2000ms，以此类推。
pub fn next_backoff_ms(attempt: u32) -> u64 {
    let base: u64 = 500;
    let cap: u64 = 8_000;
    (base * (1u64 << attempt)).min(cap)
}
