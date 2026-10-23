//! 隐私门控模块（V1-F1-S03）
//!
//! 设计对齐：设计文档§三#6 + 关键机制隐私三件套
//!
//! 职责：
//! - 持有 App 排除名单（`ExcludeList`）
//! - 定义捕获策略（`CapturePolicy`）：暂停开关 + 敏感开关 + 排除名单引用
//! - 提供 `should_skip_with_source` 纯函数：依序判定快照是否应被跳过
//!
//! 判定顺序（`should_skip_with_source`）：
//! 1. policy.paused       → Paused
//! 2. snapshot.is_concealed && policy.skip_sensitive → Concealed
//! 3. 来源应用在 exclude → Excluded（bundle id 或本地化显示名任一命中）
//! 4. snapshot.has_self_marker → SelfMark
//! 5. 否则 → None（可捕获）
//!
//! 重要约束：本模块不对剪贴板内容做任何启发式分析（不猜密码、不检测 token 等）。
//! 敏感判定仅依赖平台标记（is_concealed）和用户显式配置（排除名单）。

use std::borrow::Cow;
use std::collections::HashSet;

use crate::clipboard::{ClipboardSnapshot, SourceApp};

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
/// 持有用户配置的一组应用标识：既可以是 macOS bundle ID
/// （`com.1password.1password`），也可以是**应用显示名**（`1Password`）。
/// 设置页的输入框提示就是「应用名称（如 1Password）」，故两种写法都必须能命中。
///
/// 匹配规则：**大小写不敏感**、忽略首尾空白、逐项等价（不做子串/前缀匹配——
/// 名单是拦截规则，宽松匹配会误拦无关应用）。
///
/// 构造方式：
/// - [`ExcludeList::new_with_apps`]：从字符串迭代器批量构造（测试 / 初始化）
/// - [`ExcludeList::default`]：空名单
#[derive(Default)]
pub struct ExcludeList {
    /// 归一化（去首尾空白 + 小写折叠）后的名单条目
    apps: HashSet<String>,
}

/// 归一化匹配键：去首尾空白，含大写字母时转小写。
///
/// 无大写字母时借用原串，避免每次轮询都为常见的小写标识分配内存。
fn normalize_app_key(value: &str) -> Cow<'_, str> {
    let trimmed = value.trim();
    if trimmed.chars().any(char::is_uppercase) {
        Cow::Owned(trimmed.to_lowercase())
    } else {
        Cow::Borrowed(trimmed)
    }
}

impl ExcludeList {
    /// 名单是否为空（空名单时所有匹配都短路返回 false，零额外开销）。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.apps.is_empty()
    }

    /// 判断 `app` 是否在排除名单内（大小写不敏感）。
    ///
    /// ```rust
    /// use quickquick_lib::privacy::ExcludeList;
    /// let list = ExcludeList::new_with_apps(["com.foo.app", "1Password"]);
    /// assert!(list.contains("com.foo.app"));
    /// assert!(list.contains("1password"));
    /// assert!(!list.contains("com.bar.app"));
    /// ```
    #[must_use]
    pub fn contains(&self, app: &str) -> bool {
        if self.apps.is_empty() {
            return false;
        }
        self.apps.contains(normalize_app_key(app).as_ref())
    }

    /// 判断来源应用身份是否被排除：bundle id 或本地化显示名任一命中即排除。
    ///
    /// 空名单直接返回 false，不读取身份字段（轮询路径零额外开销）。
    #[must_use]
    pub fn matches_source(&self, source: &SourceApp) -> bool {
        if self.apps.is_empty() {
            return false;
        }
        source
            .bundle_id
            .as_deref()
            .is_some_and(|bundle_id| self.contains(bundle_id))
            || source
                .display_name
                .as_deref()
                .is_some_and(|name| self.contains(name))
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
            apps: iter
                .into_iter()
                .map(normalize_app_key)
                .map(Cow::into_owned)
                .collect(),
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

/// 判定快照是否应被跳过（仅用快照自带的单值来源标识），返回第一个命中的跳过原因。
///
/// 这是 [`should_skip_with_source`] 在"没有来源应用完整身份"时的等价入口，
/// 只匹配快照里的 bundle id / 可执行名，不做显示名匹配。
/// 剪贴板轮询路径调用的是 [`should_skip_with_source`]（它能拿到完整身份）。
///
/// 判定顺序（优先级由高到低）：
/// 1. `policy.paused`                              → [`SkipReason::Paused`]
/// 2. `snapshot.is_concealed && skip_sensitive`    → [`SkipReason::Concealed`]
/// 3. `source_app` 在 `exclude`                   → [`SkipReason::Excluded`]
/// 4. `snapshot.has_self_marker`                   → [`SkipReason::SelfMark`]
/// 5. 否则 → `None`
///
/// 本函数不分析剪贴板内容（无启发式识别）。
#[must_use]
pub fn should_skip(snapshot: &ClipboardSnapshot, policy: &CapturePolicy<'_>) -> Option<SkipReason> {
    should_skip_with_source(snapshot, policy, None)
}

/// 判定快照是否应被跳过，来源应用可用完整身份（bundle id + 本地化显示名）。
///
/// 与 [`should_skip`] 的区别只在第三条规则：排除名单会同时比对 `source_app` 字段
/// 与 `source_app` 的 bundle id / 显示名——用户按设置页提示输入「1Password」时，
/// 显示名与 bundle id 必须都能命中，否则设置页等于在撒谎。
///
/// 名单为空时第三条规则短路返回 false，不读取任何身份字段（轮询零额外开销）。
#[must_use]
pub fn should_skip_with_source(
    snapshot: &ClipboardSnapshot,
    policy: &CapturePolicy<'_>,
    source_app: Option<&SourceApp>,
) -> Option<SkipReason> {
    if policy.paused {
        return Some(SkipReason::Paused);
    }

    if snapshot.is_concealed && policy.skip_sensitive {
        return Some(SkipReason::Concealed);
    }

    if is_source_excluded(snapshot, policy, source_app) {
        return Some(SkipReason::Excluded);
    }

    if snapshot.has_self_marker {
        return Some(SkipReason::SelfMark);
    }

    None
}

/// 来源应用是否在排除名单内。
///
/// 两个来源都看：快照字段（后端给出的单值标识）与完整身份（bundle id / 显示名）。
/// 拆成独立函数是为了让 `should_skip_with_source` 保持扁平、原因清晰。
fn is_source_excluded(
    snapshot: &ClipboardSnapshot,
    policy: &CapturePolicy<'_>,
    source_app: Option<&SourceApp>,
) -> bool {
    if policy.exclude.is_empty() {
        return false;
    }
    snapshot
        .source_app
        .as_deref()
        .is_some_and(|app| policy.exclude.contains(app))
        || source_app.is_some_and(|source| policy.exclude.matches_source(source))
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

    // ===== 来源应用显示名匹配（P1-2） =====

    /// 只带显示名的身份（macOS 应用名场景，如「1Password」）。
    fn named_source(display_name: &str) -> SourceApp {
        SourceApp {
            bundle_id: None,
            display_name: Some(display_name.to_owned()),
        }
    }

    #[test]
    fn exclude_by_display_name_matches() {
        // Arrange：用户按设置页提示输入应用名，名单里没有 bundle id
        let exclude = ExcludeList::new_with_apps(["1Password"]);
        let policy = default_policy(&exclude);
        let snap = make_snapshot();
        let source = named_source("1Password");

        // Act
        let reason = should_skip_with_source(&snap, &policy, Some(&source));

        // Assert：显示名命中 → 跳过
        assert_eq!(
            reason,
            Some(SkipReason::Excluded),
            "名单里写应用名时，显示名必须能命中（否则设置页在撒谎）"
        );
    }

    #[test]
    fn exclude_by_bundle_id_matches() {
        // Arrange：名单里写 bundle id，身份两个字段都有
        let exclude = ExcludeList::new_with_apps(["com.1password.1password"]);
        let policy = default_policy(&exclude);
        let snap = make_snapshot();
        let source = SourceApp {
            bundle_id: Some("com.1password.1password".to_owned()),
            display_name: Some("1Password".to_owned()),
        };

        // Act
        let reason = should_skip_with_source(&snap, &policy, Some(&source));

        // Assert
        assert_eq!(reason, Some(SkipReason::Excluded), "bundle id 命中应跳过");
    }

    #[test]
    fn exclude_matching_is_case_insensitive() {
        // Arrange：名单大写、身份小写（用户手输与系统取值常常大小写不一致）
        let exclude = ExcludeList::new_with_apps(["1PASSWORD"]);
        let policy = default_policy(&exclude);
        let snap = make_snapshot();
        let source = named_source("1password");

        // Act & Assert
        assert_eq!(
            should_skip_with_source(&snap, &policy, Some(&source)),
            Some(SkipReason::Excluded),
            "显示名匹配应大小写不敏感"
        );
        assert!(exclude.contains("1PaSsWoRd"), "contains 也应大小写不敏感");
    }

    #[test]
    fn non_listed_app_is_not_excluded() {
        // Arrange：名单是 1Password，复制来自 TextEdit
        let exclude = ExcludeList::new_with_apps(["1Password"]);
        let policy = default_policy(&exclude);
        let snap = make_snapshot();
        let source = SourceApp {
            bundle_id: Some("com.apple.TextEdit".to_owned()),
            display_name: Some("TextEdit".to_owned()),
        };

        // Act & Assert
        assert_eq!(
            should_skip_with_source(&snap, &policy, Some(&source)),
            None,
            "名单外应用不应被排除"
        );
    }

    #[test]
    fn empty_exclude_list_never_excludes() {
        // Arrange：空名单 + 看似可匹配的身份
        let exclude = ExcludeList::default();
        let policy = default_policy(&exclude);
        let snap = make_snapshot();
        let source = named_source("1Password");

        // Act & Assert：空名单短路（不读取身份），永远不排除
        assert!(exclude.is_empty(), "新构造的空名单应为空");
        assert!(!exclude.matches_source(&source), "空名单不应匹配任何来源");
        assert_eq!(should_skip_with_source(&snap, &policy, Some(&source)), None);
    }

    #[test]
    fn snapshot_source_field_alone_still_excludes() {
        // Arrange：后端只给了单值标识（无完整身份对象）
        let exclude = ExcludeList::new_with_apps(["com.foo.secret"]);
        let policy = default_policy(&exclude);
        let mut snap = make_snapshot();
        snap.source_app = Some("com.foo.secret".to_owned());

        // Act & Assert：快照字段命中同样跳过
        assert_eq!(
            should_skip_with_source(&snap, &policy, None),
            Some(SkipReason::Excluded)
        );
    }

    #[test]
    fn concealed_takes_priority_over_excluded() {
        // Arrange：既敏感又来自名单内应用
        let exclude = ExcludeList::new_with_apps(["1Password"]);
        let policy = default_policy(&exclude);
        let mut snap = make_snapshot();
        snap.is_concealed = true;
        let source = named_source("1Password");

        // Act & Assert：先判敏感（原因更贴近实际，便于诊断日志）
        assert_eq!(
            should_skip_with_source(&snap, &policy, Some(&source)),
            Some(SkipReason::Concealed)
        );
    }

    #[test]
    fn skip_sensitive_false_falls_through_to_exclude() {
        // Arrange：用户关掉敏感跳过，但保留排除名单
        let exclude = ExcludeList::new_with_apps(["1Password"]);
        let policy = CapturePolicy {
            paused: false,
            skip_sensitive: false,
            exclude: &exclude,
        };
        let mut snap = make_snapshot();
        snap.is_concealed = true;
        let source = named_source("1Password");

        // Act & Assert：敏感不拦，但名单仍拦
        assert_eq!(
            should_skip_with_source(&snap, &policy, Some(&source)),
            Some(SkipReason::Excluded)
        );
    }
}
