---
id: TV3-retrospective
type: retrospective
level: 版本
parent: TV3
created: 2026-06-06T00:00:00Z
promoted: [TV3-RETRO-1]
---

# TV3 版本复盘

> 裁决通过后编排器写。把本版真实发生的打回/复现坑/流程摩擦逐条记下，每条标 [晋升机制]/[仅观察]/[一次性]；[晋升机制] 项喂下一版（TV4）启动前置门禁。

| 现象 | 根因 | 分类 | 晋升去向 / 处置 |
|---|---|---|---|
| **coder 自报"487 passed"但实树编译失败**：TV3-F2 coder polish 后只跑 `cargo check` 未实跑全量 `cargo test` 就收尾自报通过，实际 GeminiProvider URL `format!` 漏占位符（2 占位 3 参，exit 101），tester 命中校验首步即抓到、打回 #1 | coder 把"编译过(cargo check)"等同于"测试过"，最后一次编辑后未重跑测试套件就声明完成——违反"证据先于断言/完成定义须实跑" | **[晋升机制]** | **TV3-RETRO-1（部分落地）**：通用项，目标晋升全局 coder 契约 / code-standards §测试——「coder 声明小功能完成前，**最后一次编辑之后必须实跑全量 `cargo test`（或项目等价全量测试），不能以 `cargo check`/单测子集代替**；贴真实 `test result: ok. N passed` 行，自报 passed 数须来自本次实跑而非记忆」。**已落项目本地 hints.md；全局已落 coder agent 契约「提交前自检清单」（2026-06-06 用户批准后补落），跨项目复用生效**。已回填 promoted |
| **ChatGLM 缺 Bearer 前缀（Critical）**：build_request Authorization 头直接用裸 JWT，漏 `Bearer ` 前缀，真请求会 401；测试盲点 `matches('.').count()==2` 对裸 JWT 和 `Bearer x.y.z` 都过、抓不到。reviewer 对照智谱官方文档抓出、打回 #2 | coder 实现鉴权头时未逐字核对厂商官方文档的头格式；测试用结构性弱断言（点数）而非前缀强断言 | **[仅观察]** | reviewer 对照官方文档核鉴权头格式 + 引证来源，机制有效抓出；coder 已补 `starts_with("Bearer ")` 强断言闭合。既有"复杂签名/鉴权按厂商官方文档"红线 + reviewer 官方文档核对已覆盖根因，暂不机制化；若后续鉴权头格式错反复出现，转 [晋升机制]（去向：tester/reviewer 对鉴权头默认核前缀+官方格式） |
| **tester/coder 多次撞 maxTurns**：F1 coder×2、F1/F2/F3 tester 各撞顶续跑 | 多源 + 跨抽象（chat helper/JWT/异构 body）回合密集 | **[仅观察]** | 与 TV1/TV2 同源、续跑兜底有效无 lost work；既有"派发切小到预算内"晋升已覆盖。延续观察 |
| **裸短串测试名 0 passed 假绿风险**：测试嵌套 `translate::providers::tests::`，`cargo test 短名` 可能 0 命中却 exit 0 | cargo test 子串过滤 + 模块嵌套 | **[一次性]** | tester/producer 已用完整模块路径 + N≥1 计数规避，hints 已有 RTK 取原始输出条目覆盖。无需新增机制 |

## 晋升落地确认
- **TV3-RETRO-1 [晋升机制] 已全局落地（2026-06-06）**：项目本地 `docs/dev-log/hints.md`（coder 交付前必须实跑全量测试，cargo check 不够）即时避坑；`promoted: [TV3-RETRO-1]` 已回填。**全局已落 `~/.claude/agents/coder.md` 提交前自检清单首条（用户批准后补落），跨项目复用生效**。
- 往期未落地 [晋升机制]：TV1-RETRO-1（已落地）、TV2-RETRO-1（项目本地 + 全局 code-standards §8 均已落，2026-06-06）——无新增遗留。
- [仅观察]/[一次性] 项暂存，复发再转 [晋升机制]。
