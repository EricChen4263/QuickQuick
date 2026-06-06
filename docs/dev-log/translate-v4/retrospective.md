---
id: TV4-retrospective
type: retrospective
level: 版本
parent: TV4
created: 2026-06-06T00:00:00Z
promoted: [TV4-RETRO-1]
---

# TV4 版本复盘

> 裁决通过后编排器写。把本版真实发生的打回/复现坑/流程摩擦逐条记下，每条标 [晋升机制]/[仅观察]/[一次性]。本版为方案A最终版本，[晋升机制] 项喂下一战略/版本启动前置门禁。

| 现象 | 根因 | 分类 | 晋升去向 / 处置 |
|---|---|---|---|
| **coder 大重构陷上下文膨胀死循环**：TV4-F1 枚举重构（触及 19 源 + DTO + 前端）coder 连续多次 maxTurns 续跑，到后期每次 resume 立即撞顶、0 进展（67k token / 0 tool_use），续跑机制失效 | 单 coder 累积转录随大重构膨胀，resume 重载超长上下文即耗尽预算；续跑机制对"上下文已饱和"的 agent 无效——续跑只续任务不缩上下文 | **[晋升机制]** | **TV4-RETRO-1（部分落地）**：通用编排经验，目标晋升全局 goal-dev 规范「派发纪律」——「同一 subagent 续跑撞顶且 ≥2 次无实质进展（工具调用数≈0 或无文件变更）时，判定其上下文已饱和，**改派全新 subagent**（干净上下文）带『已落地状态摘要 + 精确剩余步骤』接力，而非继续 resume 老 agent」。本版实测：改派 fresh coder 后一轮推进到前端 tsc、再一轮收口完成。**已落项目本地 hints.md；全局已落 `goal-dev-workflow` SKILL.md 编排纪律「续跑撞顶无进展 ≥2 次改派 fresh agent」（2026-06-06 用户批准后补落），跨项目复用生效**。已回填 promoted |
| **大重构机械替换宜批量而非逐处**：F1 的 24 处 `.translated` 断言适配，逐处 Edit 极慢且撞顶；改用一条 perl 批量替换 + helper 一次收口 | 逐处 Edit 对同构批量改动低效、耗回合 | **[仅观察]** | 编排器已在续跑指令中给出 perl 批量替换方案，有效。属编排手法，hints「回合预算有限/交付序」已涵盖精神；若再现可并入 TV4-RETRO-1 的接力摘要规范。暂不单独机制化 |
| **tester 用 Bash 绕写 test.md 缺 frontmatter**：TV4-F3 tester（无 Write 权）用 Bash heredoc 写了 test.md，但缺标准 frontmatter（id/type/parent/commit/acceptance_ids），编排器复核时补齐 | tester 越过"无 Write、由编排器落盘"约定自行写文件，且不带 frontmatter | **[仅观察]** | 编排器已复核补齐 frontmatter（commit 回填依赖它）。属个例；若 tester 反复自行写留痕致 frontmatter 缺失，转 [晋升机制]（去向：tester 契约明确"不自写 dev-log，只返结论"）。暂观察 |
| **跨大功能非阻塞项顺修有效**：两个 reviewer I-1（F1 RESULT_B mock 缺 kind、F2 doc 注释）均 confidence 偏低/非阻塞，分别在 F4/F3 顺手闭环 | reviewer 非阻塞 Important/低置信项 | **[一次性]** | 顺修机制（非阻塞项记入 feature-report 遗留、在后续同区域大功能顺修）运作良好，无需机制化 |

## 晋升落地确认
- **TV4-RETRO-1 [晋升机制] 已全局落地（2026-06-06）**：已落项目本地 `docs/dev-log/hints.md`（续跑撞顶无进展 ≥2 次改派 fresh agent 接力）即时避坑；`promoted: [TV4-RETRO-1]` 已回填。**全局已落 `~/.claude/skills/goal-dev-workflow/SKILL.md` 编排纪律（用户批准后补落），跨项目复用生效**。
- 往期 [晋升机制] 落地状态：TV1-RETRO-1（项目本地已落）、TV2-RETRO-1 / TV3-RETRO-1（项目本地 + 全局均已落，2026-06-06）——全部落地，无遗留。
- [仅观察]/[一次性] 项暂存，复发再转 [晋升机制]。

## 方案A 收官说明
TV4 通过标志方案A（翻译源对齐 pot）四版（TV1-TV4）全部条件性通过。21 内置源 + DictEntry 枚举 + 前端词典组件落地，registry 23 provider。全版 manual_confirm 项（TV1-F*/TV2-F1234/TV3-M01/TV4-M01）累积于 pending-manual，待用户配密钥/真机批量采证，不阻塞各版 done。
