---
id: TV1-retrospective
type: retrospective
level: 版本
parent: TV1
created: 2026-06-06T00:00:00Z
promoted: [TV1-RETRO-1]
---

# TV1 版本复盘

> 裁决通过后编排器写。把本版真实发生的打回/复现坑/流程摩擦逐条记下，每条标 [晋升机制]/[仅观察]/[一次性]；[晋升机制] 项喂下一版（TV2）启动前置门禁。

| 现象 | 根因 | 分类 | 晋升去向 / 处置 |
|---|---|---|---|
| **移除/替换源后留下过时注释与 fixture**：F1 移除 MyMemory 后，`tests/ipc_settings.rs` 头注释、`tests/translate.rs` fixture、`mod.rs:65` `needs_key` doc 多处仍引用已删的 mymemory，分别被 F1-S01 / F2-S02 / F3-S01 reviewer 抓到、延到后续小功能才补 | coder 新增/移除聚焦"目标改动"，对"全仓清理同名旧引用（注释/fixture/doc）"覆盖不全；跨小功能反复复发（3 次） | **[晋升机制]** | **TV1-RETRO-1（已落地）**：写入项目 `docs/dev-log/hints.md` Hint 段——「移除/重命名 provider/源/实体时，全仓 `grep` 旧名，同批清理注释/fixture/doc 中的过时引用，纳入交付清单逐条核销」。TV2 还会持续新增源、命名演进，此坑高频，项目本地即时避坑 |
| **tester 撞 maxTurns**：F2-S02 / F3-S01 / F4-S01 三次 tester 在边界探测/连跑/裁决处撞顶中途停，靠 SendMessage 续跑补完 | 跨端（Rust+前端）或多源/多变异（A/B/C/D）的动态证伪回合密集，超 tester 预算上限 | **[仅观察]** | 续跑机制每次有效兜底、无 lost work（tester 落 /tmp 证据 + 编排器续派）；既有 post-v3 晋升「派发切小到预算内」已覆盖根因。暂不再机制化；若后续出现续跑也救不回的 lost work，再转 [晋升机制]（去向：编排器对跨端/多源证伪默认按档拆派——命中+变异 / 边界+连跑+裁决两档） |
| **coder 撞 maxTurns**：F1-S01 / F4-S01 coder 在集成测试收尾/跨端接入处撞顶，续跑补完 | 重任务（provider+移除+迁移 / Rust+前端跨端）回合消耗大 | **[仅观察]** | 同上，既有「coder 交付序 + 切小」晋升已覆盖；续跑兜底有效，无 lost work |
| **非官方免 key 源端点不可控**：DeepL-free（www2.deepl.com/jsonrpc）实测稳定 429 限流，无法实现 | 第三方非官方接口对匿名/本环境限流封禁，属外部现实 | **[一次性]** | 已走 acceptance change_log 暂缓留痕 + 设计§七多源兜底；coder「先 curl 实测、不通不硬造」红线有效防止了编造跑不通的实现。无需机制化 |

## 晋升落地确认
- **TV1-RETRO-1 [晋升机制] 已落地**：见 `docs/dev-log/hints.md` Hint 段新增条目，已回填本文件 `promoted: [TV1-RETRO-1]`。TV2 启动前置门禁核对：本项已落地，不阻塞开版。
- [仅观察] 项（tester/coder 撞顶）暂存，复发且致 lost work 再转 [晋升机制]。
