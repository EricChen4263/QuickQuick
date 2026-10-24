---
status: done
category: implementation
subject: Maccy 对标 · Phase 2 收尾（P2-10/P2-11）+ P3-1 前置护栏（P3-0）+ P1-8 实施判断
summary: 收掉 Phase 2 中最后两个不依赖面板语义的打磨项（行内预览分级延迟、快捷键徽标随修饰键变），并补上 P3-1 换掉 arboard 之前必须存在的捕获/粘贴链路契约测试（14 条）。同时把「P1-8 不能单独做」的实施判断写进 work items：只搬线程不改 changeCount，等于用主线程全量读换概率降低。
evidence:
  commits:
    - db717d0 feat(popover): 行内预览分级延迟（首 1500ms / 后续 200ms，P2-10）
    - 39511f4 feat(popover): 快捷键徽标随修饰键切换并补全新键位提示（P2-11）
    - f6ea50e test(capture): 补 P3-1 前置的捕获/粘贴链路契约护栏（P3-0）
    - 9887f23 fix(popover): 预览延迟期间不再显示「无选中项」误导空态（P2-10 复审修复）
    - 65b3ed5 docs(capture): 契约测试的不变量指向设计权威来源并修正因果论据（P3-0 复审同步）
  commands:
    - "cd src-tauri && cargo test"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings"
    - "pnpm exec tsc -b"
    - "pnpm test"
  urls: []
---

# AI 活动记录 · Phase 2 收尾 + P3-1 前置护栏

## 摘要

Phase 2 里不依赖面板语义的条目本轮全部做完：行内预览分级延迟（P2-10）、快捷键徽标随修饰键变并把新键位暴露出来（P2-11）。同时开了 **P3-0**——不是新功能，而是路线图为 P3-1 写死的前置：「先补集成测试覆盖现有捕获路径再动手」。

## P2-10：延迟的目的不是省请求，是别闪

本项目的预览数据本来就在内存（`PopoverPreview` 直接渲染 `ClipItem`），所以照搬「首次 1500ms / 后续 200ms」而没有想清楚目的，就会做成一个纯拖慢。真实目的是**快速翻页时预览区不乱跳**：档位判据用「本次面板会话是否已显示过预览」（不是时间戳），「只显示最后一个」靠 effect 里 `clearTimeout` + 重排实现，而不是节流。

复审发现一个用户一定能看到的问题：延迟窗口里**列表有高亮项、预览区却显示「无选中项」**，语义相反、看起来像坏了。修法不是加个三元，而是抽出 discriminated 三态（`item` / `waiting` / `empty`）+ 纯函数 `resolvePreviewState`，让「哪个态」的判定离开 JSX。修完后两个方向都被变异夹住：退回「null 一律 empty」→ 3 红；改成「null 一律 waiting」→ 2 红。

执行者还顺带发现并修掉一处 `act` 告警回归：加 `previewSession` 后，既有测试里 3 处裸调 focus 回调开始产生渲染，于是 `not wrapped in act` 从 4 条涨到 8 条。它先用 worktree 建基线确认不是自己新测试引起的（并记下一个坑：`pnpm exec vitest` 在 worktree 里会静默失败，必须用 `./node_modules/.bin/vitest` 才跑得起来，否则会把「跑不起来」误判成「0 告警」），再把那 3 处包进 `act()`——断言一条没改，告警归零。

## P2-11：徽标的优先级必须有依据

修饰键分层的坑在于「优先级拍脑袋」。执行者去读了真实分发表述，结论是 **⌘ > ⌃ > ⌥、⇧ 无独立层**：`handleMoveKey` 首行 `if (e.metaKey) return false`，所以按住 ⌘ 时 `⌃N/⌃P` 根本不生效；`⇧` 在面板里没有任何专属组合，单列一层会造出不存在的期望。复审独立核对了这条依据（没有采信注释），确认与实现一致。

默认层把「`1–0` 直达」放上去了——这是上一轮新增但没有 UI 线索的能力，discoverability 的主要缺口就在这里。

## P3-0：给「换掉 arboard」先铺安全网

P3-1 要把 `ArboardBackend::change_count()` / `read()` 换成直连 `NSPasteboard`，风险登记里标的是「高风险：破坏现有捕获/粘贴链路」。本轮补了 `src-tauri/tests/capture_contract.rs`（14 条契约测试），覆盖审计列出了 12 条真缺口，包括：

- 重复复制时「置顶刷新」的可观测结果（**P3-2 用 changeCount 后这里会变，注释已写明「改语义必须同步改本条并说明理由」**）；
- 混合图文一次捕获产生两条、且 HTML 端到端进库；
- 图片超阈值降级 + 「正好等于阈值不降级」的边界；
- 隐私三层（concealed / 来源应用显示名 / 自产标记）在 poll 层的可观察结果，**且跳过时 `last_seen_count` 仍推进**（不然会下一轮重复触发）；
- 计数回退零读取、空快照、html-only 快照、粘贴降级 `write_back_only` 的联合事实。

可失败性用变异证明：让 `poll_once_with_policy` 跳过时不推进基线 → 2 条红；删掉 `should_skip_with_source` 的 concealed 分支（模拟 P3-1 重写 `read()` 后忘记填 `is_concealed`）→ 2 条红；审查者另做 3 处（html-only 产条目、阈值 `>` 改 `>=`、图文只产 1 条）全部打红。

独立审查的关键产出不是「通过」，而是**「这 14 条没有一条堵死 P3-1 的路」**：逐条按「合理重写下是否仍应通过」判过，只有第 1 条（重复复制语义）在 P3-2 改语义时**必须有意修改**——于是把它显式挂钩进 P3-2 条目，避免未来 coder 悄悄改绿。另有两处注释被指出「论据不准确」并已修正：html-only 不变量的权威来源要指向 `quickquick-richtext-fidelity.md` 的冻结决策 4（而不是「代码里 text 是必填键」这个代码事实），以及「零读取」是**开销**保证而非「不误捕」的机制保证（后者由「不推进基线 + 不返回 clips」结构性保证）——后者若不修正，会在 P3-2 制造一个虚假约束，让人不敢加本该允许的二次确认读取。

顺带报了一条未修的低危发现：`db::top_id` 的 `ORDER BY last_modified_utc DESC LIMIT 1` 缺 `rowid DESC` 兜底，与 `list_items_with_images` / `list_ordered` 不一致，而同毫秒并列时两处「最前条目」口径**会给出相反答案**（审查用 2 万轮探针实测：并列 4904 次里 100% 不一致）。但生产无调用点、当前不可复现 flaky，且修法不是简单加一行（加 `, rowid DESC` 会让依赖它的两个既有测试红），故按审查结论**降格挂到 P4-1 子项**，不单独立项。

## P1-8：记下一个「不能单独做」的判断

P1-8（串行化全部 `NSPasteboard` 访问）原本标 ready。本轮把它重新想了一遍后改成：**必须与 P3-2 合成一次改动**。

理由：只把现有的内容哈希轮询搬到主线程，主线程每 500ms 就要做一次全量读（文本 + HTML + 图片 RGBA 哈希，一张 4K 截图 30MB+），这是用可见卡顿换「崩溃概率降低」而不是结构性消除。正确形态是改成 `NSPasteboard.changeCount`（标量，极便宜）在主线程做变化检测，只有真变了才做一次全量读——所有 pasteboard 访问天然串行，崩溃类被结构性消除。

但这条会带一个**用户可见的语义变化**（同一内容重复复制各记一条），需要用户明确接受后才动，已写进 work items 待裁决。

## 验证（Lead 侧实跑）

```
cargo check --all-targets / clippy -D warnings / fmt --check  → 全绿
cargo test                                                    → 653 passed / 0 failed
pnpm exec tsc -b                                              → 无错误
pnpm test                                                     → 67 files / 649 tests 全绿
```

## 未闭环

- **必须真机目视**：1500/200ms 的实际手感、延迟期「预览稍后显示」的措辞与样式、徽标 5 个在底部条是否溢出、按住 ⌃ 再唤面板时徽标停在默认层、点击列表项后松开 ⌃ 是否卡层。
- **Phase 2 仍 blocked**：P2-1/4/5/8/9（面板尺寸、字母 pin 的迁移、cycle、管理动作、唤起位置），全部等面板语义人工验证或迁移框架。
- **P2-10 复审修复未做二次独立审查**：改动是执行者按 review 建议实现的小修，附双向变异自证；如实标注为「自检 + 变异证据」，未走第二遍独立审查。
