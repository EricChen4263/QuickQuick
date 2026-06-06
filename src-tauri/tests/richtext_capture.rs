//! 集成测试：捕获层富文本（RT1-F1-S02）
//!
//! 验收项 RT1-F1-A02：捕获层读 HTML + 变化检测纳入 html
//! - composite_hash_differs_when_html_differs — 哈希组合纯函数区分 html
//! - snapshot_to_clips_propagates_html         — 快照→clips 透传 html 不丢

use quickquick_lib::clipboard::{snapshot_to_clips_for_test, CapturedClip, ClipboardSnapshot};
use quickquick_lib::pipeline::composite_hash_bytes;

/// A02：text 相同、html 不同 → 哈希不同；text+html 全同 → 哈希相同；
///      html None vs Some 也不同。锚定"同纯文本但新增/变更 html"可被检测。
#[test]
fn composite_hash_differs_when_html_differs() {
    let text = Some("hello");

    let plain = composite_hash_bytes(text, None, None);
    let bold = composite_hash_bytes(text, Some("<b>hello</b>"), None);
    let italic = composite_hash_bytes(text, Some("<i>hello</i>"), None);

    assert_ne!(plain, bold, "同文本但 html None vs Some 应产生不同哈希");
    assert_ne!(bold, italic, "同文本但 html 内容不同应产生不同哈希");

    let bold_again = composite_hash_bytes(text, Some("<b>hello</b>"), None);
    assert_eq!(bold, bold_again, "text+html 全同应产生相同哈希（确定性）");

    assert_eq!(
        composite_hash_bytes(None, None, None),
        None,
        "三者皆 None 时应返回 None"
    );
}

/// A02：构造带 html 的 ClipboardSnapshot → snapshot_to_clips 产出的
///      CapturedClip::Text 的 html 等于输入（锚定透传不丢）。
#[test]
fn snapshot_to_clips_propagates_html() {
    let snapshot = ClipboardSnapshot {
        text: Some("hello world".to_owned()),
        html: Some("<b>hello world</b>".to_owned()),
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: None,
    };

    let clips = snapshot_to_clips_for_test(snapshot);

    let item = clips
        .into_iter()
        .find_map(|c| match c {
            CapturedClip::Text(item) => Some(item),
            CapturedClip::Image { .. } => None,
        })
        .expect("应产出含 Text 的 CapturedClip");

    assert_eq!(
        item.html,
        Some("<b>hello world</b>".to_owned()),
        "snapshot.html 应原样透传到 CapturedItem.html"
    );
}
