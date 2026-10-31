//! trans-popover 待译文本来源（热键即译取词）IPC 层。
//!
//! 两件事：
//! - [`TransSource`]：热键唤起时 Rust 取好的「待译文本 + 来源」。它既是 `trans-source`
//!   推送事件的 payload，也是冷启动补读的返回值——两处必须是同一份数据，来源标签才不会
//!   在冷启动/暖触发之间自相矛盾。
//! - [`get_trans_popover_source`]：冷启动补读命令。前端窗口首次挂载晚于 Rust 的 emit，
//!   那一份推送会落在没人监听的时候丢掉；Rust 把同一份快照留在 [`TransSourceStore`] 里
//!   供前端挂载后补读。

use std::sync::Mutex;

use serde::Serialize;
use tauri::State;

use crate::selection::SelectionOutcome;

/// 待译文本的实际来源。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TransSourceKind {
    /// 当前选中文字。
    Selection,
    /// 剪贴板历史最新一条（取词失败时的降级）。
    Clipboard,
}

/// 取词失败、降级到剪贴板的原因。
///
/// 前端据此给出如实的降级说明：只说「来自剪贴板」会把「为什么不是选中内容」留给用户猜，
/// 而未授权辅助功能与平台不支持这两种情况，用户能采取的动作完全不同。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FallbackReason {
    /// 已授权但没读到选中文字（无焦点元素 / 应用不暴露选区 / 选区为空）。属于常态，不额外提示。
    NoSelection,
    /// 本平台没有取词通道。
    Unsupported,
    /// 未获辅助功能授权。
    Untrusted,
}

/// 一次唤起的待译文本与来源。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransSource {
    /// 待译文本；`None` 表示选中与剪贴板都没拿到可译内容。
    pub text: Option<String>,
    /// 文本实际来自哪里。
    pub source: TransSourceKind,
    /// 降级到剪贴板的原因；来源是选中时为 `None`。
    pub fallback_reason: Option<FallbackReason>,
}

/// 把取词结局翻译成「降级原因」；拿到选中文字时没有降级可言。
fn fallback_reason_of(outcome: &SelectionOutcome) -> Option<FallbackReason> {
    match outcome {
        SelectionOutcome::Text(_) => None,
        SelectionOutcome::NoSelection => Some(FallbackReason::NoSelection),
        SelectionOutcome::Unsupported => Some(FallbackReason::Unsupported),
        SelectionOutcome::Untrusted => Some(FallbackReason::Untrusted),
    }
}

/// 决定这次唤起该翻译什么：选中文字优先，取不到才回退剪贴板，两者都空则无文本。
///
/// 纯函数：不碰 AX、不碰 DB、不碰窗口，真实取词与查库结果由调用方喂进来。这样「优先顺序
/// + 来源标签」这条判据可以在无 GUI 环境断言到，而真机行为只剩「AX 能不能读」一个变量。
#[must_use]
pub fn build_trans_source(selection: SelectionOutcome, clipboard: Option<String>) -> TransSource {
    match selection {
        SelectionOutcome::Text(text) => TransSource {
            text: Some(text),
            source: TransSourceKind::Selection,
            fallback_reason: None,
        },
        outcome => TransSource {
            text: clipboard,
            source: TransSourceKind::Clipboard,
            fallback_reason: fallback_reason_of(&outcome),
        },
    }
}

/// trans-popover 最近一次唤起的来源快照。
///
/// 为什么需要它：窗口首次创建时 webview 还没挂载，Rust 在 show 之后 emit 的那一份事件
/// 会落在没人监听的时候丢掉。前端若改成自己查剪贴板补位，冷启动就会在「用户明明选中了
/// 文字」的情况下译剪贴板内容并标成「剪贴板」——正是要避免的那种误译认知。
///
/// 用 `Mutex` 而不是原子量：快照是一个含 `String` 的结构体，无法原子替换；写入只在热键
/// 唤起时发生，读取只在窗口挂载时发生一次，标准库 `Mutex` 的竞争成本可以忽略。
#[derive(Default)]
pub struct TransSourceStore(Mutex<Option<TransSource>>);

impl TransSourceStore {
    /// 记下本次唤起的来源快照，覆盖上一份（前端只关心最近一次唤起）。
    pub fn remember(&self, source: TransSource) {
        match self.0.lock() {
            Ok(mut guard) => *guard = Some(source),
            // 锁中毒说明此前的写入路径 panic 过；此时保留旧快照已无意义，只求不 panic 地继续。
            Err(e) => eprintln!("[QuickQuick] 记录 trans-popover 来源快照失败（锁中毒）: {e}"),
        }
    }

    /// 取走最近一次唤起的来源快照副本；从未唤起过时为 `None`。
    #[must_use]
    pub fn snapshot(&self) -> Option<TransSource> {
        match self.0.lock() {
            Ok(guard) => guard.clone(),
            Err(e) => {
                eprintln!("[QuickQuick] 读取 trans-popover 来源快照失败（锁中毒）: {e}");
                None
            }
        }
    }

    /// 清空快照。窗口失焦隐藏时调用：选中的文字不应在窗口关闭后继续留在内存里。
    /// 下一次热键唤起会重新写入，所以清空不影响后续翻译。
    pub fn clear(&self) {
        match self.0.lock() {
            Ok(mut guard) => *guard = None,
            Err(e) => eprintln!("[QuickQuick] 清空 trans-popover 来源快照失败（锁中毒）: {e}"),
        }
    }
}

/// 读取最近一次热键唤起的待译文本与来源。
///
/// 前端冷启动路径用它替代「自己查剪贴板」：热键唤起时 Rust 已经在窗口显示之前取好了词，
/// 前端再查一遍剪贴板只会得到来源标错的结论。
#[tauri::command]
pub fn get_trans_popover_source(state: State<'_, TransSourceStore>) -> Option<TransSource> {
    state.snapshot()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 清空后不再能读回快照：窗口隐藏后选中文字不应继续留在内存里。
    #[test]
    fn store_clear_drops_snapshot() {
        let store = TransSourceStore::default();
        store.remember(build_trans_source(
            SelectionOutcome::Text("secret".to_string()),
            None,
        ));
        assert!(store.snapshot().is_some());

        store.clear();

        assert_eq!(store.snapshot(), None, "clear 之后快照必须为空");
    }

    /// 选中优先：同时有选中文字与剪贴板内容时，译选中文字且不标降级。
    #[test]
    fn build_trans_source_prefers_selection_over_clipboard() {
        let source = build_trans_source(
            SelectionOutcome::Text("selected text".to_string()),
            Some("clipboard text".to_string()),
        );

        assert_eq!(source.text, Some("selected text".to_string()));
        assert_eq!(source.source, TransSourceKind::Selection);
        assert_eq!(source.fallback_reason, None);
    }

    /// 无选中时回退剪贴板，并如实带上「无选中」这一降级原因。
    #[test]
    fn build_trans_source_falls_back_to_clipboard_when_no_selection() {
        let source = build_trans_source(
            SelectionOutcome::NoSelection,
            Some("clipboard text".to_string()),
        );

        assert_eq!(source.text, Some("clipboard text".to_string()));
        assert_eq!(source.source, TransSourceKind::Clipboard);
        assert_eq!(source.fallback_reason, Some(FallbackReason::NoSelection));
    }

    /// 未授权辅助功能：降级剪贴板，并把「未授权」这一原因带给前端做降级说明。
    #[test]
    fn build_trans_source_marks_untrusted_fallback() {
        let source = build_trans_source(SelectionOutcome::Untrusted, Some("clip".to_string()));

        assert_eq!(source.source, TransSourceKind::Clipboard);
        assert_eq!(source.fallback_reason, Some(FallbackReason::Untrusted));
        assert_eq!(source.text, Some("clip".to_string()));
    }

    /// 平台不支持取词：降级剪贴板，原因标 Unsupported（Windows 的「暂不支持选中取词」）。
    #[test]
    fn build_trans_source_marks_unsupported_fallback() {
        let source = build_trans_source(SelectionOutcome::Unsupported, Some("clip".to_string()));

        assert_eq!(source.source, TransSourceKind::Clipboard);
        assert_eq!(source.fallback_reason, Some(FallbackReason::Unsupported));
    }

    /// 选中与剪贴板都没有可译内容：文本为空，前端据此给「请先选中或复制文字」提示。
    #[test]
    fn build_trans_source_reports_empty_text_when_both_sides_missing() {
        let source = build_trans_source(SelectionOutcome::NoSelection, None);

        assert_eq!(source.text, None);
        assert_eq!(source.source, TransSourceKind::Clipboard);
        assert_eq!(source.fallback_reason, Some(FallbackReason::NoSelection));
    }

    /// 快照：写入后可读回同一份数据；未写入时为空。
    #[test]
    fn store_round_trips_latest_snapshot() {
        let store = TransSourceStore::default();
        assert_eq!(store.snapshot(), None, "未唤起过时快照必须为空");

        store.remember(build_trans_source(
            SelectionOutcome::Text("first".to_string()),
            None,
        ));
        store.remember(build_trans_source(
            SelectionOutcome::Untrusted,
            Some("second".to_string()),
        ));

        let snapshot = store.snapshot().expect("写入后必须能读回快照");
        assert_eq!(
            snapshot.text,
            Some("second".to_string()),
            "快照只保留最近一次唤起"
        );
        assert_eq!(snapshot.fallback_reason, Some(FallbackReason::Untrusted));
    }

    /// 线上契约：字段名与枚举值必须与前端 TransSourcePayload（camelCase）逐字对齐，
    /// 跨语言无法编译期共享，只能靠这条断言把序列化结果钉住。
    #[test]
    fn trans_source_serializes_to_camel_case_wire_format() {
        let selected = build_trans_source(
            SelectionOutcome::Text("hello".to_string()),
            Some("clip".to_string()),
        );
        let json = serde_json::to_value(&selected).expect("TransSource 必须可序列化");
        assert_eq!(
            json,
            serde_json::json!({
                "text": "hello",
                "source": "selection",
                "fallbackReason": serde_json::Value::Null,
            })
        );

        let degraded = build_trans_source(SelectionOutcome::Untrusted, Some("clip".to_string()));
        let json = serde_json::to_value(&degraded).expect("TransSource 必须可序列化");
        assert_eq!(
            json,
            serde_json::json!({
                "text": "clip",
                "source": "clipboard",
                "fallbackReason": "untrusted",
            })
        );
    }
}
