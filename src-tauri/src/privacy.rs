//! 隐私门控模块（V1-F1-S03）
//!
//! 设计对齐：设计文档§三#6 + 关键机制隐私三件套
//!
//! 职责：
//! - 持有当前生效的忽略规则集（`IgnoreRules`，历史名 `ExcludeList`）：应用名单 + 当轮静默意图
//! - 定义捕获策略（`CapturePolicy`）：暂停开关 + 敏感开关 + 规则集引用
//! - 提供 `should_skip_with_source` 纯函数：依序判定快照是否应被跳过
//! - 提供托盘点击的纯决策（`tray_click_action`）与「仅忽略下一次」的一次性开关（`SkipOnce`）
//!
//! 判定顺序（`should_skip_with_source`，**顺序本身是契约，改动需同步改测试**）：
//! 1. 本轮暂停且不是「仅忽略下一次」 → Paused
//! 2. 本轮是「仅忽略下一次」        → SkipOnce
//! 3. snapshot.is_concealed && policy.skip_sensitive → Concealed
//! 4. 应用名单规则（黑名单命中）   → Excluded
//! 5. snapshot.has_self_marker      → SelfMark
//! 6. 否则 → None（可捕获）
//!
//! 顺序理由：前两条是「用户此刻的硬意图」，第三条是「系统的隐私承诺」，
//! 四五条是「用户可调规则」，最后一条是「机器事实」——自产标记只在本工具写回剪贴板时出现，
//! 放到最后不影响正确性，还能让前面的规则先给出更贴近用户意图的原因。
//!
//! 重要约束：本模块不对剪贴板内容做任何启发式分析（不猜密码、不检测 token 等）。
//! 敏感判定仅依赖平台标记（is_concealed）和用户显式配置（排除名单）。

use std::borrow::Cow;
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::clipboard::{ClipboardBackend, ClipboardSnapshot, SourceApp};

/// 跳过原因枚举。
///
/// 每个变体对应一条判定规则，便于调用方按需记录或展示原因。
/// 变体顺序即 [`should_skip_with_source`] 的判定优先级（见模块文档）。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkipReason {
    /// 用户通过托盘或设置页暂停了捕获（持久化开关）
    Paused,
    /// 用户用 `⌥⇧` 点托盘图标，只忽略**这一次**变化（一次性开关，不持久化）
    SkipOnce,
    /// 快照携带平台 concealed/transient 标记（敏感内容，如密码管理器填充）
    Concealed,
    /// 来源应用命中应用名单条目
    Excluded,
    /// 本工具自写剪贴板的私有 UTI 标记（防自污染）
    SelfMark,
}

/// 当前生效的忽略规则集。
///
/// **历史名保留**：这个类型最初只承载「App 排除名单」，现在还额外携带
/// 一个当轮静默意图（见下方 `silence` 字段）。新代码请用别名 [`IgnoreRules`]，
/// 名字与本注释都按「规则集」理解，不要再当它是一个名单。
///
/// 应用名单的条目既可以是 macOS bundle ID（`com.1password.1password`），
/// 也可以是**应用显示名**（`1Password`）。设置页的输入框提示就是「应用名称（如 1Password）」，
/// 故两种写法都必须能命中。匹配规则：**大小写不敏感**、忽略首尾空白、逐项等价
/// （不做子串/前缀匹配——名单是拦截规则，宽松匹配会误拦无关应用）。
///
/// 自身还携带一个**当轮静默意图**（`silence`）：它不参与任何判定，只影响
/// [`SkipReason`] 取 `Paused` 还是 `SkipOnce`。共享实例恒为 [`RoundSilence::None`]，
/// 只有轮询线程在「仅忽略下一次」生效的那一轮才用 [`ExcludeList::for_skip_once_round`]
/// 派生一份带标记的当轮副本。
///
/// 构造方式：
/// - [`ExcludeList::with_rules`]：从设置项完整构造（应用入口）
/// - [`ExcludeList::new_with_apps`]：只给应用名单（测试 / 只需要名单的场景）
/// - [`ExcludeList::default`]：什么都不拦
#[derive(Default)]
pub struct ExcludeList {
    /// 归一化（去首尾空白 + 小写折叠）后的应用名单条目
    apps: HashSet<String>,
    /// 当轮静默意图（只影响 `SkipReason` 取值，不改变任何判定结果）
    silence: RoundSilence,
}

/// 忽略规则集的语义化别名。
///
/// `ExcludeList` 是历史名（当年只有排除名单），语义已经扩成「当前生效的忽略规则集」。
/// 新代码请用这个名字；类型本身不改名是为了不牵动既有调用点与跨 crate 契约测试。
pub type IgnoreRules = ExcludeList;

/// 当轮静默意图：区分「暂停」与「仅忽略下一次」这两种本轮行为完全等价、
/// 但原因不同的情况。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RoundSilence {
    /// 本轮未因一次性开关静默
    #[default]
    None,
    /// 本轮静默来自托盘 `⌥⇧` 的「仅忽略下一次」
    SkipOnce,
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
    /// 应用名单是否为空（空名单时名单规则短路返回「不拦」，零额外开销）。
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.apps.is_empty()
    }

    /// 当前的当轮静默意图。
    #[must_use]
    pub fn round_silence(&self) -> RoundSilence {
        self.silence
    }

    /// 判断 `app` 是否在应用名单内（大小写不敏感）。
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

    /// 判断来源应用身份是否命中名单条目：bundle id 或本地化显示名任一命中即算命中。
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

    /// 派生一份带「仅忽略下一次」标记的当轮副本。
    ///
    /// 只在托盘 `⌥⇧` 置位且本轮确有新内容时才调用，常态轮次借用共享规则集、
    /// 不付这份克隆成本（空名单时克隆不分配内存）。
    #[must_use]
    pub fn for_skip_once_round(&self) -> Self {
        Self {
            apps: self.apps.clone(),
            silence: RoundSilence::SkipOnce,
        }
    }

    /// 从字符串切片迭代器构造只含应用名单的规则集。
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
            ..Self::default()
        }
    }
}

/// 捕获策略：聚合当前运行时的跳过规则。
///
/// 借用规则集而非持有，避免克隆开销；轮询循环可持有 policy 的短暂引用。
/// **字段语义与签名是契约**：`src-tauri/tests/**` 有大量字面量构造，
/// 新增规则请挂到 [`IgnoreRules`] 上而不是这里。
pub struct CapturePolicy<'a> {
    /// 本轮是否静默（不记录任何变化）
    ///
    /// 两个来源共用这一个字段：持久化的暂停开关，以及托盘 `⌥⇧` 的「仅忽略下一次」。
    /// 两者在本轮的可观察行为完全一致（读完快照、推进基线、不入库），
    /// 差异只在 [`SkipReason`] 的取值，由规则集的 [`RoundSilence`] 区分。
    pub paused: bool,
    /// 是否跳过平台 concealed/transient 标记的敏感内容（默认 true）
    ///
    /// false 时即使 `snapshot.is_concealed == true` 也不跳过，
    /// 允许用户主动关闭隐私保护来捕获密码管理器内容。
    pub skip_sensitive: bool,
    /// 当前生效的忽略规则集引用
    pub exclude: &'a ExcludeList,
}

/// 判定快照是否应被跳过（仅用快照自带的单值来源标识），返回第一个命中的跳过原因。
///
/// 这是 [`should_skip_with_source`] 在"没有来源应用完整身份"时的等价入口，
/// 只匹配快照里的 bundle id / 可执行名，不做显示名匹配。
/// 剪贴板轮询路径调用的是 [`should_skip_with_source`]（它能拿到完整身份）。
///
/// 判定顺序（优先级由高到低）：
/// 1. 本轮静默且非「仅忽略下一次」             → [`SkipReason::Paused`]
/// 2. 本轮静默且为「仅忽略下一次」             → [`SkipReason::SkipOnce`]
/// 3. `snapshot.is_concealed && skip_sensitive` → [`SkipReason::Concealed`]
/// 4. 应用名单规则命中拦截                     → [`SkipReason::Excluded`]
/// 5. `snapshot.has_self_marker`               → [`SkipReason::SelfMark`]
/// 6. 否则 → `None`
///
/// 本函数不分析剪贴板内容（无启发式识别）。
#[must_use]
pub fn should_skip(snapshot: &ClipboardSnapshot, policy: &CapturePolicy<'_>) -> Option<SkipReason> {
    should_skip_with_source(snapshot, policy, None)
}

/// 判定快照是否应被跳过，来源应用可用完整身份（bundle id + 本地化显示名）。
///
/// 与 [`should_skip`] 的区别只在应用名单那条规则：名单会同时比对 `source_app` 字段
/// 与完整身份的 bundle id / 显示名——用户按设置页提示输入「1Password」时，
/// 显示名与 bundle id 必须都能命中，否则设置页等于在撒谎。
///
/// 名单为空时该规则短路返回「不拦」，不读取任何身份字段（轮询零额外开销）。
#[must_use]
pub fn should_skip_with_source(
    snapshot: &ClipboardSnapshot,
    policy: &CapturePolicy<'_>,
    source_app: Option<&SourceApp>,
) -> Option<SkipReason> {
    if policy.paused {
        // 暂停与「仅忽略下一次」对本轮结果完全等价，只有原因不同；
        // 后者由轮询线程在置位那一轮用带标记的当轮规则副本表达。
        return Some(match policy.exclude.round_silence() {
            RoundSilence::SkipOnce => SkipReason::SkipOnce,
            RoundSilence::None => SkipReason::Paused,
        });
    }

    if snapshot.is_concealed && policy.skip_sensitive {
        return Some(SkipReason::Concealed);
    }

    if is_source_blocked(snapshot, policy, source_app) {
        return Some(SkipReason::Excluded);
    }

    if snapshot.has_self_marker {
        return Some(SkipReason::SelfMark);
    }

    None
}

/// 来源应用是否被应用名单规则拦下。
///
/// 两个来源都看：快照字段（后端给出的单值标识）与完整身份（bundle id / 显示名）。
/// 命中即拦；空名单 = 不排除任何应用（历史行为）。
fn is_source_blocked(
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

/// 托盘点击时刻的修饰键状态（只含本功能用到的两个键）。
///
/// 抽成与平台无关的值类型：平台层（macOS `NSEvent` / Windows `GetKeyState`）
/// 只负责把系统状态翻译成它，点击语义由 [`tray_click_action`] 这个纯函数决定，
/// 于是「⌥ / ⌥⇧ / ⇧ / 无修饰」四种组合可以在任何平台单测，无需真机托盘。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TrayModifiers {
    /// ⌥（Option / Alt）是否按下
    pub alt: bool,
    /// ⇧（Shift）是否按下
    pub shift: bool,
}

/// 点击托盘图标应执行的处置动作。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrayClickAction {
    /// 显示主窗口（默认行为）
    Show,
    /// 切换「暂停捕获」（持久化开关）
    TogglePause,
    /// 只忽略下一次剪贴板变化（一次性，不持久化）
    SkipNextOnce,
}

/// 把「点击托盘图标 + 点击时刻的修饰键」映射成处置动作。
///
/// 对齐 Maccy 的键位语义：
/// - 无修饰 → 显示主窗口
/// - `⌥`    → 切换暂停捕获
/// - `⌥⇧`   → 只忽略下一次
/// - 仅 `⇧` → 显示主窗口（没有独立语义；退回默认行为，避免用户按错键时「什么都没发生」）
///
/// ```rust
/// use quickquick_lib::privacy::{tray_click_action, TrayClickAction, TrayModifiers};
/// let alt = TrayModifiers { alt: true, shift: false };
/// assert_eq!(tray_click_action(alt), TrayClickAction::TogglePause);
/// ```
#[must_use]
pub fn tray_click_action(modifiers: TrayModifiers) -> TrayClickAction {
    match (modifiers.alt, modifiers.shift) {
        (true, true) => TrayClickAction::SkipNextOnce,
        (true, false) => TrayClickAction::TogglePause,
        _ => TrayClickAction::Show,
    }
}

/// 「仅忽略下一次」的一次性开关。
///
/// 与暂停的**关键区别**：它不写 settings.json、不改 `paused`，用掉一次就失效。
/// 状态与消费规则放在一起，是因为「什么时候才算用掉一次」正是本机制最容易写错的地方
/// （见 [`SkipOnce::take_if_new_content`]）。
#[derive(Debug, Default)]
pub struct SkipOnce {
    /// 是否已置位（等待下一次真实变化）
    armed: AtomicBool,
}

impl SkipOnce {
    /// 创建未置位的一次性开关。
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// 置位：下一次真正有内容变化时不记录。
    pub fn arm(&self) {
        self.armed.store(true, Ordering::Relaxed);
    }

    /// 当前是否置位（只读，不消费）。
    #[must_use]
    pub fn is_armed(&self) -> bool {
        self.armed.load(Ordering::Relaxed)
    }

    /// 结合「本轮是否真有新内容」决定本轮是否静默，并在真有内容时消费掉这次机会。
    ///
    /// 返回 `true` 表示本轮这次变化应被忽略（且机会已用掉）。
    ///
    /// **`has_new_content == false` 时绝不消费**：轮询是 500ms 一轮的固定节拍，
    /// 用户按完 `⌥⇧` 后很可能几轮之内都没复制任何东西；若在空轮就 `swap(false)`，
    /// 用户争取到的那一次机会会被一个「什么都没发生」的瞬间白吃掉，
    /// 之后真的复制敏感内容时反而照常入库——这正是本机制最隐蔽的错法。
    #[must_use]
    pub fn take_if_new_content(&self, has_new_content: bool) -> bool {
        if !has_new_content {
            return false;
        }
        self.armed.swap(false, Ordering::Relaxed)
    }
}

/// 判定本轮是否作为「仅忽略下一次」静默，并在确有新内容时消费掉开关。
///
/// # 为什么变化判据是 `change_count` 而不是「本轮入库结果为空」
/// 「有变化」必须先于候选策略确定：`capture_and_ingest` 内部才做变化检测，
/// 等它返回时内容已经进了库，再消费开关只能算事后补救。故这里先按与
/// `poll_once_with_policy` 完全相同的口径探一次（`current > last_seen`），
/// 只有该判据成立才消费、才让本轮静默。
///
/// # 代价
/// 置位期间每轮多探一次 `change_count()`。只有用户刚按过 `⌥⇧` 的短窗口才会走到，
/// 常态轮次直接短路在 `is_armed()`，不为一个几乎不用的开关付双倍读取成本。
///
/// # 已知边界
/// 若这次变化恰好是本工具自己写回剪贴板（自产标记），机会也会被用掉：
/// 探针看到的是「剪贴板变了」，与用户按 `⌥⇧` 时的心智一致（这一次变化确实不记）。
/// 想区分自产标记需要在本层读快照，代价是常态轮次要付一次全量读，不值得。
#[must_use]
pub fn resolve_skip_once(
    skip_once: &SkipOnce,
    backend: &dyn ClipboardBackend,
    last_seen: u64,
) -> bool {
    if !skip_once.is_armed() {
        return false;
    }
    skip_once.take_if_new_content(backend.change_count() > last_seen)
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

    // ===== 托盘点击语义（P3-6） =====

    /// 可编程变化计数的假剪贴板后端：确定性驱动轮次，不碰真实剪贴板、不 sleep。
    struct FakeBackend {
        count: std::cell::Cell<u64>,
        text: Option<&'static str>,
    }

    impl FakeBackend {
        fn with_count(count: u64) -> Self {
            Self {
                count: std::cell::Cell::new(count),
                text: Some("copied text"),
            }
        }

        fn set_count(&self, count: u64) {
            self.count.set(count);
        }
    }

    impl ClipboardBackend for FakeBackend {
        fn change_count(&self) -> u64 {
            self.count.get()
        }

        fn read(&self) -> ClipboardSnapshot {
            ClipboardSnapshot {
                text: self.text.map(str::to_owned),
                html: None,
                image: None,
                has_self_marker: false,
                is_concealed: false,
                source_app: None,
            }
        }
    }

    #[test]
    fn tray_click_without_modifiers_shows_window() {
        assert_eq!(
            tray_click_action(TrayModifiers::default()),
            TrayClickAction::Show
        );
    }

    #[test]
    fn tray_click_with_option_toggles_pause() {
        let modifiers = TrayModifiers {
            alt: true,
            shift: false,
        };
        assert_eq!(tray_click_action(modifiers), TrayClickAction::TogglePause);
    }

    #[test]
    fn tray_click_with_option_and_shift_skips_next_once() {
        let modifiers = TrayModifiers {
            alt: true,
            shift: true,
        };
        assert_eq!(tray_click_action(modifiers), TrayClickAction::SkipNextOnce);
    }

    #[test]
    fn tray_click_with_shift_only_falls_back_to_show() {
        // ⇧ 没有独立语义：退回默认「显示」，避免按错键时完全没反应
        let modifiers = TrayModifiers {
            alt: false,
            shift: true,
        };
        assert_eq!(tray_click_action(modifiers), TrayClickAction::Show);
    }

    #[test]
    fn skip_once_consumed_only_after_real_capture_change() {
        // Arrange：⌥⇧ 置位，后端随后产生一次真实变化
        let skip_once = SkipOnce::new();
        skip_once.arm();
        let backend = FakeBackend::with_count(1);
        let mut last_seen = 0u64;
        let rules = ExcludeList::default();

        // Act 第一轮：计数递增 → 本轮静默并消费掉机会
        let first_silent = resolve_skip_once(&skip_once, &backend, last_seen);
        let round_rules = first_silent.then(|| rules.for_skip_once_round());
        let policy = CapturePolicy {
            paused: first_silent,
            skip_sensitive: true,
            exclude: round_rules.as_ref().unwrap_or(&rules),
        };
        let first_clips =
            crate::clipboard::poll_once_with_policy(&backend, &mut last_seen, &policy);

        // Assert 第一轮：静默、零入库，但基线必须推进（否则下一轮会重复看到同一次变化）
        assert!(first_silent, "置位且计数递增的那一轮必须静默");
        assert!(first_clips.is_empty(), "第一次变化不该产生任何入库条目");
        assert_eq!(
            last_seen, 1,
            "跳过时基线仍要推进，否则下一轮会重复看到同一次变化"
        );
        assert!(!skip_once.is_armed(), "一次变化之后开关必须已失效");

        // Act 第二轮：内容再次变化，但机会已用掉
        backend.set_count(2);
        let second_silent = resolve_skip_once(&skip_once, &backend, last_seen);
        let policy = CapturePolicy {
            paused: second_silent,
            skip_sensitive: true,
            exclude: &rules,
        };
        let second_clips =
            crate::clipboard::poll_once_with_policy(&backend, &mut last_seen, &policy);

        // Assert 第二轮：恢复正常入库
        assert!(!second_silent, "第二次变化必须恢复正常入库");
        assert_eq!(second_clips.len(), 1, "第二次捕获应正常产生一条待入库条目");
    }

    #[test]
    fn skip_once_survives_round_without_new_content() {
        // Arrange：⌥⇧ 置位，且本轮计数与基线相同（这一轮什么都没复制）
        let skip_once = SkipOnce::new();
        skip_once.arm();
        let backend = FakeBackend::with_count(7);
        let last_seen = 7u64;

        // Act：空轮
        let silent_on_empty_round = resolve_skip_once(&skip_once, &backend, last_seen);

        // Assert：空轮不能把一次性机会吃掉
        assert!(!silent_on_empty_round, "空轮不该静默");
        assert!(
            skip_once.is_armed(),
            "空轮绝不能消费掉机会——否则用户按完 ⌥⇧ 干等几轮就白按了"
        );

        // Act：随后真的产生变化
        backend.set_count(8);
        let silent_on_real_change = resolve_skip_once(&skip_once, &backend, last_seen);

        // Assert：机会仍然有效
        assert!(silent_on_real_change, "空轮之后的第一次真实变化应被忽略");
        assert!(!skip_once.is_armed(), "真实变化之后才失效");
    }

    #[test]
    fn skip_once_round_reports_skip_once_reason() {
        // Arrange：带「仅忽略下一次」标记的当轮规则副本
        let rules = ExcludeList::default();
        let round_rules = rules.for_skip_once_round();
        let policy = CapturePolicy {
            paused: true,
            skip_sensitive: true,
            exclude: &round_rules,
        };

        // Act & Assert：本轮静默且原因是 SkipOnce，而不是 Paused
        assert_eq!(
            should_skip(&make_snapshot(), &policy),
            Some(SkipReason::SkipOnce)
        );
        assert_eq!(
            rules.round_silence(),
            RoundSilence::None,
            "共享规则集不能被标记，标记只存在于当轮副本上"
        );
    }

    #[test]
    fn paused_round_reports_paused_reason() {
        // Arrange：持久化暂停（共享规则集，无当轮标记）
        let rules = ExcludeList::default();
        let policy = CapturePolicy {
            paused: true,
            skip_sensitive: true,
            exclude: &rules,
        };

        // Act & Assert
        assert_eq!(
            should_skip(&make_snapshot(), &policy),
            Some(SkipReason::Paused)
        );
    }
}
