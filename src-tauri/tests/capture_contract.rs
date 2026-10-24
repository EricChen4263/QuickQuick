//! 集成测试：捕获/粘贴链路契约护栏
//!
//! 目的：P3-1「换掉 arboard，直连 `NSPasteboard`」会重写
//! `ArboardBackend::change_count()` / `read()` 及其下游的「捕获 → 入库」链路。
//! 本文件在动手替换**之前**，把「替换之后必须保持不变的可观察行为」用断言钉死，
//! 让 P3-1 的改动能靠 `cargo test` 判死，而不是靠人肉回归。
//!
//! 全部用例不碰真实系统剪贴板、不 sleep、不依赖真实时钟：
//! 剪贴板后端一律注入 fake，库一律用 tempfile + 固定密钥的 fake `KeyProvider`。
//!
//! # 覆盖审计（机械枚举，三类）
//!
//! ## A. 已有覆盖（本文件不重复断言，仅登记出处，避免重复写）
//!
//! - 轮询计数驱动 / 无变化 / 自写标记 / 暂停：
//!   `tests/clipboard.rs::{capture_dual_field, poll_changecount_triggers_capture,
//!   self_write_marker_skipped, poll_count_reset_defense, pause_stops_capture,
//!   pause_false_captures_normally}`、
//!   `tests/capture_image.rs::{poll_text_only, poll_image_only, poll_text_and_image,
//!   poll_self_marker_returns_empty, poll_privacy_skip_returns_empty,
//!   poll_no_change_returns_empty}`
//! - 内容去重 / 置顶（`db` 层与 `capture_and_ingest` 层）：
//!   `tests/clipboard.rs::{dedup_and_bump, bump_no_new_record}`、
//!   `tests/boot_pipeline.rs::boot_pipeline_dedup_bumped`、
//!   `tests/richtext_storage.rs::plaintext_dedup_unchanged_on_encrypted_store`
//! - 开库 / 入库可见 / 无变化返回空：
//!   `tests/boot_pipeline.rs::{boot_pipeline_open_db, boot_pipeline_ingest_visible,
//!   boot_pipeline_no_change_none}`
//! - 图文一次捕获产生 2 个结果 + 入库原子回滚：
//!   `tests/capture_image.rs::{capture_and_ingest_text_and_image,
//!   capture_and_ingest_rolls_back_on_partial_failure, capture_and_ingest_image_only}`
//!   （前者只断言 `outcomes.len() == 2` 与「按 kind 数行数」，未断言两条 outcome 各自可见、
//!   也未断言 html 是否真的进了库）
//! - RGBA ↔ PNG 编解码：`tests/capture_image.rs::{rgba_to_png_valid_encodes_decodable_png,
//!   rgba_to_png_bad_length_returns_none, png_to_rgba_roundtrips_known_pixels,
//!   png_to_rgba_invalid_bytes_returns_err}`
//! - HTML 参与变化检测 / 快照透传 / 库内 roundtrip：
//!   `tests/richtext_capture.rs::{composite_hash_differs_when_html_differs,
//!   snapshot_to_clips_propagates_html}`、
//!   `tests/richtext_storage.rs::fresh_db_persists_richtext_roundtrip_through_encrypted_store`
//!   （两段各自成立，但「捕获 → 入库 → 列表可见」这条组合路径没有测试）
//! - 图片超大降级：`tests/image.rs::oversize_skip_original_policy_configurable`
//!   （测的是 `image::ingest_image_with_policy` 原语层，不是捕获链路）
//! - 隐私纯谓词：`tests/privacy.rs::{concealed_skipped, concealed_no_heuristic,
//!   app_exclude_list, app_not_in_exclude_list, app_exclude_none_source}` +
//!   `src/privacy.rs` 内联 15 条（`should_skip_with_source` 的判定顺序与双匹配）
//! - 粘贴写回时序与降级：`tests/paste.rs` 6 条 +
//!   `src/ipc/system.rs` 内联 `map_outcome_*` / `paste_orchestrate_*` /
//!   `paste_image_orchestrate_*`（共 18 条内联）
//! - 焦点让出决策：`src/popover.rs` 内联 `paste_focus_plan_*` / `should_inject_paste_*`
//!   （4 条，`cfg(target_os = "macos")`）
//! - 图片总量清理触发判据：`src/pipeline.rs` 内联 5 条
//! - 生产后端 trait 可实例化：`tests/macos_backends.rs` 2 条
//!
//! ## B. P3-1 会踩到、但此前**没有测试**的缺口（本文件补齐）
//!
//! - B1 同内容重复复制时「置顶刷新」的可观测结果：现有用例只断言行数不变，
//!   没有断言既有行的 `last_modified_utc` 真的被刷新（这是 P3-2 会改变语义的接缝）。
//! - B2 混合图文两条 `IngestOutcome` 各自可见 + html 端到端进库并出现在列表里。
//! - B3 图片超阈值降级在**捕获链路**（poll → PNG 编码 → `db::ingest_image_as_clip`
//!   → `clip_images`）上的结果，且条目仍在列表中可见。
//! - B4 阈值边界：等于阈值保留原图、超 1 字节降级（链路层，非原语层）。
//! - B5 `concealed` 与「来源应用显示名排除」在 `poll_once_with_policy` 层的可观察结果。
//! - B6 `ClipboardBackend::source_app_identity()` 这条默认方法的重写：全仓此前
//!   没有任何测试覆盖它，而 `ArboardBackend` 正是在 `read()` 里缓存身份、由它回放。
//! - B7 「跳过导致下一轮重复触发」的机制护栏：用 `read()` 调用次数证明基线确实推进。
//! - B8 计数回退分支**不读内容**（现只有「返回空 + 基线下调」，没有零读取保证）。
//! - B9 计数递增但快照内容为空（清空剪贴板 / 无可识别类型）→ 空 Vec 且基线推进。
//! - B10 `snapshot_to_clips` 的空快照；以及 html-only 快照不单独成条
//!   （依据 `docs/design/quickquick-richtext-fidelity.md` 冻结决策 4 的
//!   「`text_hash` 只对纯文本」，不是代码巧合）。
//! - B11 pipeline 层「concealed 永不入库」的端到端护栏。
//! - B12 粘贴降级后「内容确实已在剪贴板里」这一联合事实（现有内联单测只断言
//!   outcome 字符串与「未 send_paste」）。
//!
//! ## C. 无法自动化（pending-manual，明确不做自动化）
//!
//! - C1 `ArboardBackend::new/change_count/read` 的真实 arboard + 系统剪贴板行为。
//! - C2 真机上「同一内容重复复制时 arboard 层内容哈希不变 ⇒ 不产生第二次捕获」
//!   这一现网行为：本文件只能在入库侧锁契约（见 B1），arboard 侧必须真机观察。
//! - C3 `is_concealed` 的真实平台标记（macOS `ConcealedType` / Windows 剪贴板格式）。
//! - C4 焦点交出失败时 `paste_to_front` 返回 `write_back_only` 的**接线**
//!   （需 `AppHandle` + 真实焦点，headless 不可达）。纯决策由 `src/popover.rs` 内联覆盖。
//! - C5 图片写回剪贴板（`MacOsPasteBackend::write_image`）的真实 NSPasteboard 行为。
//! - C6 从真实富文本应用复制时的 HTML 保真度。

use std::cell::{Cell, RefCell};

use quickquick_lib::clipboard::{
    observe_backend, poll_once, poll_once_with_policy, snapshot_to_clips_for_test, CapturedClip,
    ClipboardBackend, ClipboardSnapshot, RawImageData, SourceApp,
};
use quickquick_lib::db::{self, IngestOutcome};
use quickquick_lib::image::{get_image_original, get_original_present};
use quickquick_lib::ipc::system::paste_orchestrate;
use quickquick_lib::keyprovider::{KeyError, KeyProvider};
use quickquick_lib::onboarding::AccessibilityProbe;
use quickquick_lib::paste::PasteBackend;
use quickquick_lib::pipeline;
use quickquick_lib::privacy::{CapturePolicy, ExcludeList};
use rusqlite::Connection;
use tempfile::TempDir;

// ===================== 测试夹具 =====================

/// 固定密钥的 fake `KeyProvider`：不触碰真实钥匙串，保证用例可重复。
struct FixedKeyProvider;

impl KeyProvider for FixedKeyProvider {
    fn get_or_create_key(&self) -> Result<[u8; 32], KeyError> {
        Ok([42u8; 32])
    }
}

/// 建一个临时加密库，返回（持有 TempDir 的守卫，连接）。
///
/// `TempDir` 必须在返回值里一起交给调用方，否则目录在测试中途就被删掉。
fn temp_db() -> (TempDir, Connection) {
    let dir = tempfile::tempdir().expect("tempdir 创建失败");
    let db_path = dir.path().join("quickquick.db");
    let conn = pipeline::open_app_db(&FixedKeyProvider, &db_path).expect("open_app_db 应成功");
    (dir, conn)
}

/// 可编程的假剪贴板后端。
///
/// - `count` / `snapshot` 可在测试中途替换，用来驱动「同一后端的多轮轮询」；
/// - `read_calls` 统计 `read()` 次数：这是「跳过/无变化时是否真的没读内容」的唯一证据；
/// - `identity` 用于覆盖 `source_app_identity()` 这条此前无测试的通道。
struct FakeBackend {
    count: Cell<u64>,
    snapshot: RefCell<ClipboardSnapshot>,
    identity: Option<SourceApp>,
    read_calls: Cell<u32>,
}

impl FakeBackend {
    /// 纯文本快照。
    fn text(count: u64, text: &str) -> Self {
        Self::with_snapshot(
            count,
            ClipboardSnapshot {
                text: Some(text.to_owned()),
                html: None,
                image: None,
                has_self_marker: false,
                is_concealed: false,
                source_app: None,
            },
        )
    }

    /// 任意快照。
    fn with_snapshot(count: u64, snapshot: ClipboardSnapshot) -> Self {
        Self {
            count: Cell::new(count),
            snapshot: RefCell::new(snapshot),
            identity: None,
            read_calls: Cell::new(0),
        }
    }

    /// 覆盖 `source_app_identity()` 的返回值（模拟后端缓存的完整来源身份）。
    fn with_identity(mut self, identity: SourceApp) -> Self {
        self.identity = Some(identity);
        self
    }

    fn set_count(&self, count: u64) {
        self.count.set(count);
    }

    fn read_calls(&self) -> u32 {
        self.read_calls.get()
    }
}

impl ClipboardBackend for FakeBackend {
    fn change_count(&self) -> u64 {
        self.count.get()
    }

    fn read(&self) -> ClipboardSnapshot {
        self.read_calls.set(self.read_calls.get() + 1);
        self.snapshot.borrow().clone()
    }

    fn source_app_identity(&self) -> Option<SourceApp> {
        self.identity.clone()
    }
}

/// 默认策略：未暂停、跳过敏感、可传入排除名单。
fn policy_with<'a>(exclude: &'a ExcludeList) -> CapturePolicy<'a> {
    CapturePolicy {
        paused: false,
        skip_sensitive: true,
        exclude,
    }
}

/// 确定性伪随机 RGBA（LCG，不读系统熵源/时钟）。
///
/// 用它而不用纯色图，是为了让 PNG 体积足够大且可复现，
/// 从而能用「小阈值」稳定造出超阈值场景，不必真的造几十 MB 数据。
fn deterministic_rgba(width: usize, height: usize, seed: u32) -> Vec<u8> {
    let mut state = seed;
    let mut bytes = Vec::with_capacity(width * height * 4);
    for _ in 0..(width * height) {
        for _ in 0..3 {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            bytes.push((state >> 24) as u8);
        }
        bytes.push(255);
    }
    bytes
}

/// 取「该 RGBA 编码成 PNG 后的字节数」，供阈值边界用例精确设定阈值。
fn png_len_of(width: usize, height: usize, rgba: &[u8]) -> u64 {
    let png = quickquick_lib::clipboard::rgba_to_png_for_test(width, height, rgba)
        .expect("合法 RGBA 应能编码为 PNG");
    png.len() as u64
}

/// 构造图片快照。
fn image_snapshot(width: usize, height: usize, rgba: Vec<u8>) -> ClipboardSnapshot {
    ClipboardSnapshot {
        text: None,
        html: None,
        image: Some(RawImageData {
            width,
            height,
            bytes: rgba,
        }),
        has_self_marker: false,
        is_concealed: false,
        source_app: None,
    }
}

// ===================== B1：重复复制的现有语义（P3-2 语义变更点）=====================

/// 同一文本被连续复制两次（两次计数递增）时，现有语义是「去重 + Bump（置顶刷新）」，
/// 而不是「产生新条目」。
///
/// 为什么这条是去重语义的锚：macOS 的变化检测改用真实 `NSPasteboard.changeCount` 后，
/// 「同一内容复制两次」会真的产生第二次计数递增并走到 `db::ingest`；本断言钉死此时的
/// 入库结果仍是「命中同 hash → Bump 置顶」，而不是新增一行。
/// （非 macOS 仍由内容指纹模拟计数，同内容二次复制根本不会触发第二次捕获。）
///
/// 本断言锁定的入库契约：不新增行、返回 `Bumped(原 id)`、既有行的 `last_modified_utc`
/// 被刷新。若有意改变去重语义，必须同步改本条并说明理由。
#[test]
fn repeated_identical_text_copy_dedups_and_bumps() {
    // Arrange：先入库一条，再把它的时间戳压成哨兵值 1，
    // 使「是否真的 bump」可以被确定性观测（bump 会写入真实 epoch ms，必然 != 1）。
    let (_dir, conn) = temp_db();
    let backend = FakeBackend::text(1, "反复复制的同一段文本");
    let mut last_seen = 0u64;
    let exclude = ExcludeList::default();
    let policy = policy_with(&exclude);

    let first = pipeline::capture_and_ingest(
        &backend,
        &mut last_seen,
        &conn,
        &policy,
        &pipeline::IngestLimits::default(),
    )
    .expect("首次捕获应成功");
    let id_a = match first.as_slice() {
        [IngestOutcome::Inserted(id)] => id.clone(),
        other => panic!("首次捕获应为单个 Inserted，实际: {other:?}"),
    };
    conn.execute(
        "UPDATE clip_items SET last_modified_utc = 1 WHERE id = ?1",
        rusqlite::params![id_a],
    )
    .expect("压哨兵时间戳应成功");

    // 再插一条别的干扰条目，让 A 在库里不是「唯一一行」，避免行数断言恒真。
    backend.snapshot.replace(ClipboardSnapshot {
        text: Some("另一条无关内容".to_owned()),
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: None,
    });
    backend.set_count(2);
    let second = pipeline::capture_and_ingest(
        &backend,
        &mut last_seen,
        &conn,
        &policy,
        &pipeline::IngestLimits::default(),
    )
    .expect("第二次捕获应成功");
    assert!(
        matches!(second.as_slice(), [IngestOutcome::Inserted(_)]),
        "干扰条目应为 Inserted，实际: {second:?}"
    );
    let b_ts: i64 = conn
        .query_row(
            "SELECT last_modified_utc FROM clip_items WHERE id != ?1 AND is_deleted = 0",
            rusqlite::params![id_a],
            |row| row.get(0),
        )
        .expect("取干扰条目时间戳应成功");

    // Act：同一内容第二次复制（计数再次递增，内容与首次完全相同）
    backend.snapshot.replace(ClipboardSnapshot {
        text: Some("反复复制的同一段文本".to_owned()),
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: None,
    });
    backend.set_count(3);
    let third = pipeline::capture_and_ingest(
        &backend,
        &mut last_seen,
        &conn,
        &policy,
        &pipeline::IngestLimits::default(),
    )
    .expect("第三次捕获应成功");

    // Assert 1：命中同内容 → Bumped 且 id 就是原行，不是新行
    assert!(
        matches!(third.as_slice(), [IngestOutcome::Bumped(id)] if *id == id_a),
        "同一内容再次复制应为 Bumped(原 id)，实际: {third:?}"
    );

    // Assert 2：行数仍是 2（没有产生新条目）
    assert_eq!(
        db::count_live(&conn).expect("count_live 应成功"),
        2,
        "去重语义要求「重复复制不产生新条目」，行数必须保持 2"
    );

    // Assert 3：置顶刷新真的发生了——哨兵值被写回真实时间戳，
    // 且新时间戳不早于任何既有行（置顶的实现就是刷新 last_modified_utc）。
    let a_ts: i64 = conn
        .query_row(
            "SELECT last_modified_utc FROM clip_items WHERE id = ?1",
            rusqlite::params![id_a],
            |row| row.get(0),
        )
        .expect("取 A 时间戳应成功");
    assert_ne!(
        a_ts, 1,
        "重复复制必须刷新既有行的 last_modified_utc（Bump/置顶），而不是原样不动"
    );
    assert!(
        a_ts >= b_ts,
        "置顶刷新后 A 的 last_modified_utc（{a_ts}）不应早于其它条目（{b_ts}）"
    );
}

// ===================== B2：混合图文一次捕获产生两条 =====================

/// 一次「混合图文」复制（文本 + HTML + 图片）必须产生**两个各自可见**的
/// `IngestOutcome`，并在列表里表现为一条富文本条目 + 一条图片条目；
/// HTML 必须原样进库（`html_content`）——这条组合路径此前没有任何测试。
///
/// 为什么 P3-1 会踩到：`read()` 被重写成 `types()` 全量遍历后，
/// 同一份 pasteboard 会同时给出 string / html / png 三类；只要漏掉其中任一类，
/// 或把两条 outcome 合成一条，前端就会「只看到图片」或「丢格式」。
#[test]
fn mixed_text_and_image_capture_yields_two_visible_clips() {
    // Arrange：一次复制同时带 text、html 与图片
    let (_dir, conn) = temp_db();
    let rgba = deterministic_rgba(4, 4, 7);
    let backend = FakeBackend::with_snapshot(
        1,
        ClipboardSnapshot {
            text: Some("带格式的图文".to_owned()),
            html: Some("<b>带格式的图文</b>".to_owned()),
            image: Some(RawImageData {
                width: 4,
                height: 4,
                bytes: rgba,
            }),
            has_self_marker: false,
            is_concealed: false,
            source_app: None,
        },
    );
    let mut last_seen = 0u64;
    let exclude = ExcludeList::default();
    let policy = policy_with(&exclude);

    // Act
    let outcomes = pipeline::capture_and_ingest(
        &backend,
        &mut last_seen,
        &conn,
        &policy,
        &pipeline::IngestLimits::default(),
    )
    .expect("图文混合捕获应成功");

    // Assert 1：两条 outcome 都可见，且都是「新增行」
    assert_eq!(
        outcomes.len(),
        2,
        "混合图文一次捕获必须产生两个 IngestOutcome（文本 + 图片）"
    );
    assert!(
        matches!(&outcomes[0], IngestOutcome::Inserted(_)),
        "第 0 条（文本）应为 Inserted，实际: {:?}",
        outcomes[0]
    );
    assert!(
        matches!(&outcomes[1], IngestOutcome::Inserted(_)),
        "第 1 条（图片）应为 Inserted，实际: {:?}",
        outcomes[1]
    );
    assert_ne!(
        outcomes[0], outcomes[1],
        "文本与图片必须是两个不同的条目 id，不能被合并成一条"
    );

    // Assert 2：列表里两条都可见，kind 与 html 各自正确
    let rows = db::list_items_with_images(&conn).expect("list_items_with_images 应成功");
    assert_eq!(rows.len(), 2, "列表应同时出现文本条目与图片条目");

    let rich = rows
        .iter()
        .find(|row| row.kind == "richtext")
        .expect("应有一条 kind=richtext 的文本条目");
    assert_eq!(
        rich.content, "带格式的图文",
        "富文本条目的 content 应为纯文本键"
    );
    assert_eq!(
        rich.html_content.as_deref(),
        Some("<b>带格式的图文</b>"),
        "HTML 必须端到端保真进库；丢格式是 P3-1 最可能的回归"
    );

    let image = rows
        .iter()
        .find(|row| row.kind == "image")
        .expect("应有一条 kind=image 的图片条目");
    assert!(
        image.image_id.is_some(),
        "图片条目必须关联到 clip_images 行（前端靠 image_id 拉原图）"
    );
    assert!(
        image.thumbnail.as_ref().is_some_and(|t| !t.is_empty()),
        "图片条目必须带非空缩略图，否则列表里会是一条空白行"
    );
}

// ===================== B3/B4：图片阈值降级与边界（捕获链路层）=====================

/// 原图超过 `max_image_bytes` 时，捕获链路必须只存缩略图
/// （`original_present = 0`），且该条目仍然出现在列表里。
///
/// 为什么 P3-1 会踩到：阈值由 `poll` 之后的 `ingest_image_as_clip` 透传，
/// 换掉后端时最容易发生的就是「PNG 编码后的字节数口径」变了
/// （例如改成对 TIFF 原字节判大小），导致好好的图片被静默降级。
#[test]
fn oversize_image_keeps_thumbnail_and_stays_listed() {
    // Arrange：32×32 确定性噪点图，先把它的 PNG 体量算出来，再把阈值设小 1 字节
    let (_dir, conn) = temp_db();
    let rgba = deterministic_rgba(32, 32, 11);
    let png_len = png_len_of(32, 32, &rgba);
    assert!(
        png_len > 64,
        "用例前提：确定性噪点图的 PNG 应远大于 64 字节，实际 {png_len}"
    );
    let backend = FakeBackend::with_snapshot(1, image_snapshot(32, 32, rgba));
    let mut last_seen = 0u64;
    let exclude = ExcludeList::default();
    let policy = policy_with(&exclude);

    // Act：阈值 = PNG 体积 - 1，必然超阈值
    let limits = pipeline::IngestLimits {
        max_image_bytes: png_len - 1,
        ..Default::default()
    };
    let outcomes = pipeline::capture_and_ingest(&backend, &mut last_seen, &conn, &policy, &limits)
        .expect("超大图捕获应成功（降级而非失败）");

    // Assert 1：仍然入库成功（降级不等于丢条目）
    assert!(
        matches!(outcomes.as_slice(), [IngestOutcome::Inserted(_)]),
        "超大图仍应正常入库一条，实际: {outcomes:?}"
    );

    // Assert 2：列表可见该条目，且缩略图非空
    let rows = db::list_items_with_images(&conn).expect("list_items_with_images 应成功");
    assert_eq!(rows.len(), 1, "降级后条目仍应出现在列表中");
    let image_id = rows[0]
        .image_id
        .clone()
        .expect("降级条目仍必须关联 clip_images 行（缩略图在那里）");
    assert!(
        rows[0].thumbnail.as_ref().is_some_and(|t| !t.is_empty()),
        "降级条目必须保留缩略图，否则列表里是一条空白行"
    );

    // Assert 3：原图被跳过
    assert_eq!(
        get_original_present(&conn, &image_id).expect("get_original_present 应成功"),
        Some(0),
        "超过 max_image_bytes 时必须只存缩略图（original_present=0）"
    );
    assert!(
        get_image_original(&conn, &image_id)
            .expect("get_image_original 应成功")
            .is_none_or(|original| original.is_empty()),
        "降级条目的原图 BLOB 必须为空"
    );
}

/// 阈值边界：PNG 体积**正好等于** `max_image_bytes` 时不得降级（判据是严格大于）。
///
/// 为什么值得单独钉：P3-1 若把判据写成 `>=`，用户配置「20 MB」时
/// 恰好 20 MB 的图会被无故丢掉原图，且这种偏差只出现在边界上，人工极难发现。
#[test]
fn image_at_exact_size_limit_keeps_original() {
    // Arrange
    let (_dir, conn) = temp_db();
    let rgba = deterministic_rgba(32, 32, 23);
    let png_len = png_len_of(32, 32, &rgba);
    let backend = FakeBackend::with_snapshot(1, image_snapshot(32, 32, rgba));
    let mut last_seen = 0u64;
    let exclude = ExcludeList::default();
    let policy = policy_with(&exclude);

    // Act：阈值恰好等于 PNG 体积
    let limits = pipeline::IngestLimits {
        max_image_bytes: png_len,
        ..Default::default()
    };
    pipeline::capture_and_ingest(&backend, &mut last_seen, &conn, &policy, &limits)
        .expect("边界捕获应成功");

    // Assert：原图保留
    let rows = db::list_items_with_images(&conn).expect("list_items_with_images 应成功");
    let image_id = rows[0].image_id.clone().expect("应有 clip_images 行");
    assert_eq!(
        get_original_present(&conn, &image_id).expect("get_original_present 应成功"),
        Some(1),
        "体积正好等于阈值时不应降级（判据必须是「严格大于」）"
    );
    let original = get_image_original(&conn, &image_id)
        .expect("get_image_original 应成功")
        .expect("未降级时原图应存在");
    assert_eq!(
        original.len() as u64,
        png_len,
        "保留的原图应就是捕获侧编码出的 PNG 字节"
    );
}

// ===================== B5/B6/B7：隐私三层跳过在 poll 层的可观察结果 =====================

/// 隐私三层（concealed / 来源应用排除 / 自产标记）在 `poll_once_with_policy` 层
/// 必须满足同一组可观察结论：返回空 Vec、**基线照常推进**、且不产生第二轮重复读取。
///
/// 为什么「基线推进」是关键：基线不推进时，下一轮轮询会再次读到同一份被跳过的内容；
/// 一旦下游把它当成新变化（例如 P3-1 换成真实 changeCount 后判定逻辑更宽松），
/// 被排除的内容就会漏进历史。用 `read()` 调用次数把这一点变成可断言的机制。
///
/// 其中「来源应用显示名排除」走的是 `ClipboardBackend::source_app_identity()`，
/// 这条默认方法此前在全仓没有任何测试覆盖，而 `ArboardBackend` 正是靠它在 `read()`
/// 里缓存身份 —— P3-1 重写 `read()` 时最容易连这条侧通道一起丢掉。
#[test]
fn privacy_skips_return_nothing_but_advance_baseline() {
    // Arrange：三种跳过原因的 snapshots + 身份配置
    let concealed = ClipboardSnapshot {
        text: Some("看起来很正常的内容".to_owned()),
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: true,
        source_app: None,
    };
    let excluded_by_name = ClipboardSnapshot {
        text: Some("来自被排除应用的内容".to_owned()),
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: None,
    };
    let self_mark = ClipboardSnapshot {
        text: Some("本工具自写的内容".to_owned()),
        html: None,
        image: None,
        has_self_marker: true,
        is_concealed: false,
        source_app: None,
    };
    let identity = SourceApp {
        bundle_id: Some("com.agilebits.onepassword7".to_owned()),
        display_name: Some("1Password".to_owned()),
    };
    // 名单里只写人类可读的显示名（设置页提示「应用名称（如 1Password）」），
    // 迫使匹配必须走 source_app_identity() 的显示名分支。
    let exclude = ExcludeList::new_with_apps(["1Password"]);

    let cases: [(&str, FakeBackend); 3] = [
        (
            "concealed 平台标记",
            FakeBackend::with_snapshot(5, concealed),
        ),
        (
            "来源应用显示名排除",
            FakeBackend::with_snapshot(6, excluded_by_name).with_identity(identity),
        ),
        ("自产标记", FakeBackend::with_snapshot(7, self_mark)),
    ];

    for (label, backend) in cases {
        let policy = policy_with(&exclude);
        let mut last_seen = backend.change_count() - 1;

        // Act 1：本轮计数递增，但命中跳过规则
        let clips = poll_once_with_policy(&backend, &mut last_seen, &policy);

        // Assert 1：跳过 → 空 Vec（不入库）
        assert!(
            clips.is_empty(),
            "{label}：命中跳过规则时必须返回空 Vec，实际捕获 {} 条",
            clips.len()
        );
        // Assert 2：基线照常推进（否则下一轮会重复触发）
        assert_eq!(
            last_seen,
            backend.change_count(),
            "{label}：即使跳过也必须把 last_seen_count 推进到当前计数"
        );
        // Assert 3：本轮确实读过快照（跳过是「读了之后才判掉」，不是压根没读）
        assert_eq!(
            backend.read_calls(),
            1,
            "{label}：跳过判定必须在读取快照之后，否则无法识别敏感/排除/自产内容"
        );

        // Act 2：计数未变的下一轮
        let second = poll_once_with_policy(&backend, &mut last_seen, &policy);

        // Assert 4：不重复触发——基线推进的机制证据是「这一轮根本没读内容」
        assert!(second.is_empty(), "{label}：第二轮不应再次产生捕获");
        assert_eq!(
            backend.read_calls(),
            1,
            "{label}：基线未推进会导致下一轮重复读取同一份被跳过的内容"
        );
    }
}

/// pipeline 层护栏：`concealed` 内容永不入库，且不产生任何行。
///
/// 为什么 P3-1 一定有风险：`is_concealed` 由 `read()` 从平台元数据填充。
/// 若重写后忘记把它填进快照（或恒为 `false`），密码管理器写出的内容会被静默入库，
/// 这是「隐私承诺」级别的回归，必须在替换前钉死。
#[test]
fn concealed_clip_never_reaches_database() {
    // Arrange：敏感快照 + 空库
    let (_dir, conn) = temp_db();
    let backend = FakeBackend::with_snapshot(
        1,
        ClipboardSnapshot {
            text: Some("SENTINEL_DEADBEEF_SECRET".to_owned()),
            html: None,
            image: None,
            has_self_marker: false,
            is_concealed: true,
            source_app: None,
        },
    );
    let mut last_seen = 0u64;
    let exclude = ExcludeList::default();
    let policy = policy_with(&exclude);

    // Act
    let outcomes = pipeline::capture_and_ingest(
        &backend,
        &mut last_seen,
        &conn,
        &policy,
        &pipeline::IngestLimits::default(),
    )
    .expect("敏感内容被跳过不是错误，capture_and_ingest 应返回 Ok");

    // Assert 1：没有任何入库结果、库里零行
    assert!(outcomes.is_empty(), "敏感内容不应产生任何 IngestOutcome");
    assert_eq!(
        db::count_live(&conn).expect("count_live 应成功"),
        0,
        "敏感内容绝不能进库（这是隐私承诺，不是可选项）"
    );

    // Assert 2：内容本身也没以任何形式落库
    let hit: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clip_items WHERE content LIKE '%SENTINEL_DEADBEEF_SECRET%'",
            [],
            |row| row.get(0),
        )
        .expect("查询应成功");
    assert_eq!(hit, 0, "敏感报文内容不得以任何形式出现在库里");

    // Assert 3：基线推进，下一轮不再读取
    assert_eq!(last_seen, 1, "跳过也必须推进 last_seen_count");
    pipeline::capture_and_ingest(
        &backend,
        &mut last_seen,
        &conn,
        &policy,
        &pipeline::IngestLimits::default(),
    )
    .expect("第二轮应成功");
    assert_eq!(
        backend.read_calls(),
        1,
        "基线推进后第二轮不应再次读取被跳过的内容"
    );
}

// ===================== B8/B9：计数重置防御与空内容 =====================

/// `change_count` 回退到更小值时：不误捕、不读取内容、把基线下调到新值；
/// 随后计数恢复时该正常捕获的变化不受影响。
///
/// # 「不误捕」与「零读取」是两条独立的保证，不要耦合
///
/// - **不误捕**由结构保证：回退分支既不推进基线、也不返回任何 clips，
///   与读不读内容无关；
/// - **零读取**是**开销**保证：500ms 轮询不该为一次计数回退付全量读取的代价。
///
/// 这条区分对 P3-1/P3-2 有直接后果：换成真实 `changeCount` 后，若有人想增加
/// 一次读取来区分「OS 重启归零」与「计数抖动」，那是允许的——该改动只会让本测试的
/// 读取计数断言变红并要求同步说明，不构成语义回归。
///
/// 为什么 P3-1 会踩到：换成真实 `NSPasteboard.changeCount` 后，回退分支的语义
/// 从「OS 重启归零」变成「真实可能发生的计数抖动」。回退时若读了内容并推进基线，
/// 会把一份旧内容当成新内容重复入库。
#[test]
fn count_reset_lowers_baseline_without_reading_and_recovers() {
    // Arrange：基线 10，后端计数回退到 7
    let backend = FakeBackend::text(7, "回退期间的内容");
    let mut last_seen = 10u64;
    let exclude = ExcludeList::default();
    let policy = policy_with(&exclude);

    // Act 1：计数回退
    let on_reset = poll_once_with_policy(&backend, &mut last_seen, &policy);

    // Assert 1：不误捕、基线下调
    assert!(on_reset.is_empty(), "计数回退时不应捕获任何内容");
    assert_eq!(last_seen, 7, "计数回退时应把基线下调到当前值");
    assert_eq!(
        backend.read_calls(),
        0,
        "计数回退时必须零读取：这是 500ms 轮询的开销保证（「不误捕」由「不推进基线 + 不返回 clips」结构性保证，与是否读取无关）"
    );

    // Act 2：计数恢复到 8（严格大于新基线 7）→ 这是真实变化，应正常捕获一次
    backend.set_count(8);
    let recovered = poll_once_with_policy(&backend, &mut last_seen, &policy);

    // Assert 2：恢复正常捕获，且只读一次
    assert!(
        matches!(recovered.as_slice(), [CapturedClip::Text(item)] if item.text == "回退期间的内容"),
        "基线修复后第一次真实变化应正常捕获，实际: {recovered:?}"
    );
    assert_eq!(last_seen, 8, "捕获后基线应推进到 8");
    assert_eq!(backend.read_calls(), 1, "恢复后的变化只应读一次内容");

    // Act 3：同计数再来一轮
    let no_dup = poll_once_with_policy(&backend, &mut last_seen, &policy);

    // Assert 3：不重复捕获、不再读取
    assert!(no_dup.is_empty(), "相同计数不应重复捕获");
    assert_eq!(
        backend.read_calls(),
        1,
        "计数未变时不得重复读取内容（每 500ms 一次全量读取正是 P3-2 要消除的开销）"
    );
}

/// 计数递增但快照没有任何可识别内容（例如剪贴板被清空）时：
/// 返回空 Vec，但基线必须推进，避免下一轮重复读取同一份空内容。
///
/// 为什么 P3-1 会踩到：`types()` 全量遍历后「有变化但类型都不在白名单里」
/// 会变成常见情形（P3-3 的类型白名单会让它更常见）。这条路径此前没有测试。
#[test]
fn empty_snapshot_after_change_advances_baseline_without_clips() {
    // Arrange：计数递增，但快照三字段全空
    let backend = FakeBackend::with_snapshot(
        4,
        ClipboardSnapshot {
            text: None,
            html: None,
            image: None,
            has_self_marker: false,
            is_concealed: false,
            source_app: None,
        },
    );
    let mut last_seen = 3u64;
    let exclude = ExcludeList::default();
    let policy = policy_with(&exclude);

    // Act
    let clips = poll_once_with_policy(&backend, &mut last_seen, &policy);

    // Assert
    assert!(clips.is_empty(), "空快照不应产生任何条目");
    assert_eq!(last_seen, 4, "空快照也必须推进基线，否则下一轮重复读取");
    assert_eq!(backend.read_calls(), 1, "空快照本轮应读一次以确认无内容");
}

// ===================== B10：snapshot_to_clips 边界 =====================

/// 三字段全空的快照 → 空 Vec（不 panic、不产生占位条目）。
///
/// P3-1 用 `types()` 遍历后，「读到了变化但没读到内容」会变成正常输入，
/// 这条拆分函数必须对此保持健壮。
#[test]
fn snapshot_to_clips_empty_snapshot_yields_no_clips() {
    // Arrange
    let snapshot = ClipboardSnapshot {
        text: None,
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: None,
    };

    // Act
    let clips = snapshot_to_clips_for_test(snapshot);

    // Assert
    assert!(
        clips.is_empty(),
        "空快照必须产出空 Vec，实际产出 {} 条",
        clips.len()
    );
}

/// 只有 HTML、没有纯文本的快照 → 空 Vec。
///
/// 依据是**已冻结的产品设计决策**，不是当前代码的巧合：
/// `docs/design/quickquick-richtext-fidelity.md` 的冻结决策 4 明确
/// 「纯文本相同即去重（维持现有 `text_hash` 语义）」——`text_hash` 只对纯文本计算，
/// 所以纯文本是条目得以存在的前提，html 是依附于它的保真附加字段。
///
/// P3-1 用 `types()` 全量遍历后，「有 HTML 但无 string」会从罕见变成常见；
/// 届时看到这条测试变红，第一反应不该是「遍历到了就该存」。支持 html-only 条目是
/// **产品语义变更**（会悄悄多出一批 content 为空、搜索命不中、列表显示空行的条目），
/// 必须先修改上述设计文档并重新冻结决策，再改本测试；
/// 不得仅因 P3-1 遍历到该类型就在捕获层就地产条目。
#[test]
fn snapshot_to_clips_html_without_text_yields_no_clips() {
    // Arrange：只有 html
    let snapshot = ClipboardSnapshot {
        text: None,
        html: Some("<b>只有富文本</b>".to_owned()),
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: None,
    };

    // Act
    let clips = snapshot_to_clips_for_test(snapshot);

    // Assert：不单独成条（html 依附于 text 存在）
    assert!(
        clips.is_empty(),
        "无纯文本时不应单独产生条目（text 是必填键），实际: {clips:?}"
    );
}

/// 对照用例：只有纯文本时仍正常产出 1 条 Text。
///
/// 与上一条成对存在，避免「html-only 返回空」被恒真的空实现糊过去
/// （若 `snapshot_to_clips` 退化成永远返回空 Vec，本条会红）。
#[test]
fn snapshot_to_clips_text_only_yields_one_clip() {
    // Arrange
    let snapshot = ClipboardSnapshot {
        text: Some("纯文本内容".to_owned()),
        html: None,
        image: None,
        has_self_marker: false,
        is_concealed: false,
        source_app: None,
    };

    // Act
    let clips = snapshot_to_clips_for_test(snapshot);

    // Assert
    assert_eq!(clips.len(), 1, "纯文本快照应恰好产出 1 条条目");
    assert!(
        matches!(&clips[0], CapturedClip::Text(item) if item.text == "纯文本内容" && item.html.is_none()),
        "唯一元素应为 Text，实际: {clips:?}"
    );
}

// ===================== B12：粘贴降级决策 =====================

/// 假粘贴后端：`freeze_count = true` 时写入不递增计数，用来模拟
/// 「写回成功但 `changeCount` 未确认」这一真实存在的失败形态。
struct FakePasteBackend {
    count: u64,
    clipboard_text: Option<String>,
    paste_sent: bool,
    freeze_count: bool,
}

impl FakePasteBackend {
    fn new(initial_count: u64, freeze_count: bool) -> Self {
        Self {
            count: initial_count,
            clipboard_text: None,
            paste_sent: false,
            freeze_count,
        }
    }
}

impl PasteBackend for FakePasteBackend {
    fn change_count(&self) -> u64 {
        self.count
    }

    fn write_with_marker(&mut self, item: &quickquick_lib::clipboard::CapturedItem) {
        self.clipboard_text = Some(item.text.clone());
        if !self.freeze_count {
            self.count += 1;
        }
    }

    fn current_text(&self) -> Option<String> {
        self.clipboard_text.clone()
    }

    fn send_paste(&mut self) {
        self.paste_sent = true;
    }

    fn write_image(&mut self, _width: usize, _height: usize, _rgba: &[u8]) -> Result<(), String> {
        // 文本降级路径不涉及图片写入
        Ok(())
    }
}

/// 固定授权状态的假探针。
struct FixedProbe {
    trusted: bool,
}

impl AccessibilityProbe for FixedProbe {
    fn is_trusted(&self) -> bool {
        self.trusted
    }
}

/// 写回成功但 `changeCount` 未确认时，粘贴必须降级为 `write_back_only`
/// 且**绝不注入** `⌘V`，同时内容要真的留在剪贴板里（降级是可用降级，不是无声失败）。
///
/// 为什么 P3-1 会踩到：这条判据是「不能粘出旧内容」的最后一道闸门。
/// P3-1/P3-2 若把确认逻辑改成「写入即认为成功」，这里会变成
/// 「界面报 full_paste、实际粘的是前台旧内容」——用户可见且无法察觉。
///
/// 注：另一半「焦点未交出」的分支（`should_inject_paste`）需要真实焦点，属 pending-manual；
/// 其纯决策已由 `src/popover.rs` 内联用例覆盖。
#[test]
fn paste_degrades_to_write_back_only_when_count_unconfirmed() {
    // Arrange：已授权，但写入后计数不再递增（OS 未反映写入）
    let probe = FixedProbe { trusted: true };
    let mut backend = FakePasteBackend::new(5, true);
    let item = quickquick_lib::clipboard::CapturedItem {
        text: "待粘贴的内容".to_owned(),
        html: None,
    };

    // Act
    let outcome = paste_orchestrate(&probe, &mut backend, &item);

    // Assert 1：降级标识
    assert_eq!(
        outcome, "write_back_only",
        "changeCount 未确认时必须降级为 write_back_only"
    );
    // Assert 2：绝不注入粘贴（否则会粘出前台旧内容）
    assert!(
        !backend.paste_sent,
        "未能确认写入时绝不能 send_paste（会把前台旧剪贴板内容粘出去）"
    );
    // Assert 3：降级仍要可用——内容确实已写进剪贴板，用户手动 ⌘V 能粘对
    assert_eq!(
        backend.current_text(),
        Some("待粘贴的内容".to_owned()),
        "降级路径必须保证内容已在剪贴板里，否则是无声失败"
    );
}

/// 对照用例：写入被确认时仍走完整粘贴，避免上一条被「永远降级」的实现糊过去。
#[test]
fn paste_injects_when_count_confirms_write() {
    // Arrange：已授权且写入会使计数递增
    let probe = FixedProbe { trusted: true };
    let mut backend = FakePasteBackend::new(5, false);
    let item = quickquick_lib::clipboard::CapturedItem {
        text: "正常粘贴".to_owned(),
        html: None,
    };

    // Act
    let outcome = paste_orchestrate(&probe, &mut backend, &item);

    // Assert
    assert_eq!(outcome, "full_paste", "写入被确认时应走完整粘贴");
    assert!(backend.paste_sent, "完整粘贴路径必须调用 send_paste");
}

// ===================== 编译期护栏：轮询入口未被同名替换 =====================

/// `poll_once`（无策略版）仍必须存在并保持同样的基线推进语义。
///
/// 这条存在的原因：P3-1 很可能把两套轮询入口合并成一个。
/// 合并本身可以接受，但不能出现「旧入口悄悄失去基线推进」——
/// 那会让所有还在调用它的路径反复重读同一份内容。
#[test]
fn poll_once_advances_baseline_for_self_marker() {
    // Arrange：自产标记（本工具自己写回剪贴板留下的内容）
    let backend = FakeBackend::with_snapshot(
        9,
        ClipboardSnapshot {
            text: Some("本工具写回的内容".to_owned()),
            html: None,
            image: None,
            has_self_marker: true,
            is_concealed: false,
            source_app: None,
        },
    );
    let mut last_seen = 8u64;

    // Act
    let clips = poll_once(&backend, &mut last_seen);

    // Assert
    assert!(clips.is_empty(), "自产标记条目不应被捕获（防自污染）");
    assert_eq!(
        last_seen, 9,
        "poll_once 跳过时同样必须推进基线，避免反复读取自己写入的内容"
    );
}

// ===================== 无锁观察：changeCount 变了才读内容 =====================

/// 计数没变时：不读内容、没有观察结果（轮询线程连数据库锁都不用取）。
///
/// 这是「changeCount 取代每轮全量读」的承重断言：若检测被改成恒真，
/// 每 500ms 都会读一遍文本 + HTML + 图片，`read_calls` 就不再是 0。
#[test]
fn observe_unchanged_count_returns_none_and_never_reads() {
    // Arrange：基线与当前计数相同
    let backend = FakeBackend::text(5, "未变化的内容");

    // Act
    let observed = observe_backend(&backend, 5);

    // Assert
    assert!(observed.is_none(), "计数未变不应产生观察结果");
    assert_eq!(backend.read_calls(), 0, "计数未变不得读取内容");
}

/// 计数前进时：只读一次，并把快照与来源身份原样交给后续判定。
#[test]
fn observe_advanced_count_reads_once_and_replays_snapshot_and_identity() {
    // Arrange
    let backend = FakeBackend::text(6, "新复制的内容").with_identity(SourceApp {
        bundle_id: Some("com.example.editor".to_owned()),
        display_name: Some("Editor".to_owned()),
    });

    // Act
    let observed = observe_backend(&backend, 5).expect("计数前进应产生观察结果");

    // Assert：观察结果自身就是一个只回放既成事实的后端
    assert_eq!(backend.read_calls(), 1, "一次变化只读一次内容");
    assert_eq!(observed.change_count(), 6);
    assert_eq!(
        observed.source_app_identity().and_then(|a| a.display_name),
        Some("Editor".to_owned()),
        "来源身份应随观察一起回放"
    );
    let snapshot = observed.read();
    assert_eq!(snapshot.text.as_deref(), Some("新复制的内容"));
}

/// 计数回退：不读内容，只带计数回去让后续逻辑下调基线。
#[test]
fn observe_regressed_count_does_not_read_and_lets_baseline_drop() {
    // Arrange
    let backend = FakeBackend::text(2, "重置后的内容");
    let mut last_seen = 9u64;

    // Act
    let observed = observe_backend(&backend, last_seen).expect("回退也应交给后续判定");
    let clips = poll_once_with_policy(
        &observed,
        &mut last_seen,
        &policy_with(&ExcludeList::default()),
    );

    // Assert
    assert_eq!(backend.read_calls(), 0, "计数回退不得读取内容");
    assert!(clips.is_empty(), "计数回退不应捕获任何内容");
    assert_eq!(last_seen, 2, "基线应下调到新计数");
}

/// 读取失败（串行线程忙 / 超时）：没有观察结果，调用方不推进基线，下一轮自然重试。
#[test]
fn observe_failed_read_returns_none_so_baseline_is_not_advanced() {
    // Arrange：计数前进但全量读失败
    struct FailingReadBackend;
    impl ClipboardBackend for FailingReadBackend {
        fn change_count(&self) -> u64 {
            8
        }
        fn read(&self) -> ClipboardSnapshot {
            unreachable!("read_observed 已被覆盖为失败，不应再走默认 read")
        }
        fn read_observed(&self, _seen: u64) -> Option<(u64, ClipboardSnapshot)> {
            None
        }
    }

    // Act & Assert
    assert!(
        observe_backend(&FailingReadBackend, 7).is_none(),
        "读取失败不能被当成「剪贴板为空」吞掉"
    );
}

/// 读取时刻的计数以 `read_observed` 返回值为准：读取期间剪贴板又变了，
/// 基线只推进到「读到的那一版」，下一轮会再读一次，而不是把中途变化吞掉。
#[test]
fn observe_uses_count_at_read_time_as_baseline() {
    // Arrange：轮询时计数 10，读取瞬间计数已是 11
    struct RacingBackend;
    impl ClipboardBackend for RacingBackend {
        fn change_count(&self) -> u64 {
            10
        }
        fn read(&self) -> ClipboardSnapshot {
            unreachable!("read_observed 已被覆盖")
        }
        fn read_observed(&self, _seen: u64) -> Option<(u64, ClipboardSnapshot)> {
            Some((
                11,
                ClipboardSnapshot {
                    text: Some("读取瞬间的内容".to_owned()),
                    html: None,
                    image: None,
                    has_self_marker: false,
                    is_concealed: false,
                    source_app: None,
                },
            ))
        }
    }
    let mut last_seen = 9u64;

    // Act
    let observed = observe_backend(&RacingBackend, last_seen).expect("应有观察结果");
    let exclude = ExcludeList::default();
    let clips = poll_once_with_policy(&observed, &mut last_seen, &policy_with(&exclude));

    // Assert
    assert_eq!(clips.len(), 1, "读到的内容应被捕获");
    assert_eq!(
        last_seen, 11,
        "基线应推进到读取时刻的计数，而不是轮询时的 10"
    );
}

/// 端到端：同一文本连续两次复制（计数各递增一次）经「观察 → 入库」两阶段，
/// 仍是「只留一行并置顶」——去重语义没有因为检测机制改变而变。
#[test]
fn observe_then_ingest_keeps_dedup_and_bump_semantics() {
    // Arrange
    let (_dir, conn) = temp_db();
    let backend = FakeBackend::text(1, "同一段文本");
    let exclude = ExcludeList::default();
    let policy = policy_with(&exclude);
    let limits = pipeline::IngestLimits::default();
    let mut last_seen = 0u64;

    // Act：第一次复制
    let first_obs = observe_backend(&backend, last_seen).expect("首次应有观察");
    let first = pipeline::capture_and_ingest(&first_obs, &mut last_seen, &conn, &policy, &limits)
        .expect("首次入库应成功");
    // 第二次复制：计数递增、内容相同
    backend.set_count(2);
    let second_obs = observe_backend(&backend, last_seen).expect("二次复制计数递增应有观察");
    let second = pipeline::capture_and_ingest(&second_obs, &mut last_seen, &conn, &policy, &limits)
        .expect("二次入库应成功");
    // 第三轮：计数不变，不应再读
    let reads_before_idle = backend.read_calls();
    let idle = observe_backend(&backend, last_seen);

    // Assert
    let id = match first.as_slice() {
        [IngestOutcome::Inserted(id)] => id.clone(),
        other => panic!("首次应为 Inserted，实际 {other:?}"),
    };
    assert_eq!(
        second,
        vec![IngestOutcome::Bumped(id)],
        "二次复制应命中原行并置顶"
    );
    let rows: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM clip_items WHERE is_deleted = 0",
            [],
            |r| r.get(0),
        )
        .expect("数行应成功");
    assert_eq!(rows, 1, "去重：同一文本只留一行");
    assert!(idle.is_none(), "计数不变的空轮不应产生观察结果");
    assert_eq!(backend.read_calls(), reads_before_idle, "空轮不得再读内容");
}
