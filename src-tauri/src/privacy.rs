//! 隐私门控模块（V1-F1-S03）
//!
//! 设计对齐：设计文档§三#6 + 关键机制隐私三件套
//!
//! 职责：
//! - 持有 App 排除名单（`ExcludeList`）
//! - 定义捕获策略（`CapturePolicy`）：暂停开关 + 敏感开关 + 排除名单引用
//! - 提供 `should_skip` 纯函数：依序判定快照是否应被跳过
//!
//! 判定顺序（`should_skip`）：
//! 1. policy.paused       → Paused
//! 2. snapshot.is_concealed && policy.skip_sensitive → Concealed
//! 3. source_app 在 exclude → Excluded
//! 4. snapshot.has_self_marker → SelfMark
//! 5. 否则 → None（可捕获）
//!
//! 重要约束：本模块不对剪贴板内容做任何启发式分析（不猜密码、不检测 token 等）。
//! 敏感判定仅依赖平台标记（is_concealed）和用户显式配置（排除名单）。

use std::collections::HashSet;

use crate::clipboard::ClipboardSnapshot;

/// 跳过原因枚举。
///
/// 每个变体对应一条判定规则，便于调用方按需记录或展示原因。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// 用户通过托盘暂停了捕获
    Paused,
    /// 快照携带平台 concealed/transient 标记（敏感内容，如密码管理器填充）
    Concealed,
    /// 来源应用在用户配置的排除名单内
    Excluded,
    /// 本工具自写剪贴板的私有 UTI 标记（防自污染）
    SelfMark,
}

/// App 排除名单。
///
/// 持有一组应用标识字符串（如 macOS bundle ID `com.1password.1password`）。
/// 来自排除名单内 app 的剪贴板写入将被跳过，不记录到历史。
///
/// 构造方式：
/// - [`ExcludeList::new_with_apps`]：从字符串迭代器批量构造（测试 / 初始化）
/// - [`ExcludeList::default`]：空名单
#[derive(Default)]
pub struct ExcludeList {
    apps: HashSet<String>,
}

impl ExcludeList {
    /// 判断 `app` 是否在排除名单内。
    pub fn contains(&self, app: &str) -> bool {
        self.apps.contains(app)
    }

    /// 从字符串切片迭代器构造排除名单。
    ///
    /// ```rust
    /// use quickquick_lib::privacy::ExcludeList;
    /// let list = ExcludeList::new_with_apps(["com.foo.app", "com.bar.app"]);
    /// assert!(list.contains("com.foo.app"));
    /// ```
    pub fn new_with_apps<'a, I: IntoIterator<Item = &'a str>>(iter: I) -> Self {
        Self {
            apps: iter.into_iter().map(str::to_owned).collect(),
        }
    }
}

/// 捕获策略：聚合当前运行时的跳过规则。
///
/// 借用 `ExcludeList` 而非持有，避免克隆开销；轮询循环可持有 policy 的短暂引用。
pub struct CapturePolicy<'a> {
    /// 是否处于暂停状态（用户通过托盘暂停捕获）
    pub paused: bool,
    /// 是否跳过平台 concealed/transient 标记的敏感内容（默认 true）
    ///
    /// false 时即使 `snapshot.is_concealed == true` 也不跳过，
    /// 允许用户主动关闭隐私保护来捕获密码管理器内容。
    pub skip_sensitive: bool,
    /// App 排除名单引用
    pub exclude: &'a ExcludeList,
}

/// 判定快照是否应被跳过，返回第一个命中的跳过原因；`None` 表示可捕获。
///
/// 判定顺序（优先级由高到低）：
/// 1. `policy.paused`                              → [`SkipReason::Paused`]
/// 2. `snapshot.is_concealed && skip_sensitive`    → [`SkipReason::Concealed`]
/// 3. `source_app` 在 `exclude`                   → [`SkipReason::Excluded`]
/// 4. `snapshot.has_self_marker`                   → [`SkipReason::SelfMark`]
/// 5. 否则 → `None`
///
/// 本函数不分析剪贴板内容（无启发式识别）。
pub fn should_skip(snapshot: &ClipboardSnapshot, policy: &CapturePolicy<'_>) -> Option<SkipReason> {
    if policy.paused {
        return Some(SkipReason::Paused);
    }

    if snapshot.is_concealed && policy.skip_sensitive {
        return Some(SkipReason::Concealed);
    }

    if let Some(app) = &snapshot.source_app {
        if policy.exclude.contains(app) {
            return Some(SkipReason::Excluded);
        }
    }

    if snapshot.has_self_marker {
        return Some(SkipReason::SelfMark);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clipboard::ClipboardSnapshot;

    fn make_snapshot() -> ClipboardSnapshot {
        ClipboardSnapshot {
            text: Some("hello".to_string()),
            html: None,
            image: None,
            has_self_marker: false,
            is_concealed: false,
            source_app: None,
        }
    }

    fn default_policy<'a>(exclude: &'a ExcludeList) -> CapturePolicy<'a> {
        CapturePolicy {
            paused: false,
            skip_sensitive: true,
            exclude,
        }
    }

    #[test]
    fn paused_always_skips_regardless_of_content() {
        let exclude = ExcludeList::default();
        let policy = CapturePolicy {
            paused: true,
            skip_sensitive: true,
            exclude: &exclude,
        };
        let snap = make_snapshot();
        assert_eq!(should_skip(&snap, &policy), Some(SkipReason::Paused));
    }

    #[test]
    fn concealed_skipped_when_skip_sensitive_true() {
        let exclude = ExcludeList::default();
        let policy = default_policy(&exclude);
        let mut snap = make_snapshot();
        snap.is_concealed = true;
        assert_eq!(should_skip(&snap, &policy), Some(SkipReason::Concealed));
    }

    #[test]
    fn concealed_not_skipped_when_skip_sensitive_false() {
        let exclude = ExcludeList::default();
        let policy = CapturePolicy {
            paused: false,
            skip_sensitive: false,
            exclude: &exclude,
        };
        let mut snap = make_snapshot();
        snap.is_concealed = true;
        assert_eq!(should_skip(&snap, &policy), None);
    }

    #[test]
    fn excluded_app_skips() {
        let exclude = ExcludeList::new_with_apps(["com.1password.1password"]);
        let policy = default_policy(&exclude);
        let mut snap = make_snapshot();
        snap.source_app = Some("com.1password.1password".to_string());
        assert_eq!(should_skip(&snap, &policy), Some(SkipReason::Excluded));
    }

    #[test]
    fn self_marker_skips() {
        let exclude = ExcludeList::default();
        let policy = default_policy(&exclude);
        let mut snap = make_snapshot();
        snap.has_self_marker = true;
        assert_eq!(should_skip(&snap, &policy), Some(SkipReason::SelfMark));
    }

    #[test]
    fn normal_snapshot_not_skipped() {
        let exclude = ExcludeList::default();
        let policy = default_policy(&exclude);
        let snap = make_snapshot();
        assert_eq!(should_skip(&snap, &policy), None);
    }

    #[test]
    fn paused_takes_priority_over_concealed() {
        let exclude = ExcludeList::default();
        let policy = CapturePolicy {
            paused: true,
            skip_sensitive: true,
            exclude: &exclude,
        };
        let mut snap = make_snapshot();
        snap.is_concealed = true;
        assert_eq!(should_skip(&snap, &policy), Some(SkipReason::Paused));
    }
}
