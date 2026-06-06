---
id: TV2-retrospective
type: retrospective
level: 版本
parent: TV2
created: 2026-06-06T00:00:00Z
promoted: [TV2-RETRO-1]
---

# TV2 版本复盘

> 裁决通过后编排器写。把本版真实发生的打回/复现坑/流程摩擦逐条记下，每条标 [晋升机制]/[仅观察]/[一次性]；[晋升机制] 项喂下一版（TV3）启动前置门禁。

| 现象 | 根因 | 分类 | 晋升去向 / 处置 |
|---|---|---|---|
| **「不泄露密钥」测试用空凭据，无判别力**：F2-S01 reviewer 抓到缺字段测试以空 token/apikey 构造，断言「输出不含密钥」恒真——空值本就不会出现在任何输出里，测不出真实泄露；另缺错误类型（Auth/ServerError/Quota）断言。已补强为「传入可识别脏值再断言 !contains」+ 错误类型断言后闭合 | tester/coder 写安全类断言时凭直觉用空值占位，未意识到「证否泄露」必须让敏感值真实存在于输入才有判别力 | **[晋升机制]** | **TV2-RETRO-1（部分落地）**：本属可跨项目复用的通用项，目标晋升全局 `code-standards` §8——「断言敏感值不泄露到输出/日志/错误时，凭据字段必须填入可识别的非空 sentinel 脏值再断言 `!contains(sentinel)`；空值占位恒真假绿」。已落项目本地 `docs/dev-log/hints.md`（对本项目 TV3/TV4 即时避坑）；**全局已落 `code-standards` §8（2026-06-06 用户批准后补落，含「非空 sentinel 脏值」+「独立复算锚定」+「断言验具体值不旁路」）** |
| **复杂签名「自测自证」风险**：腾讯 TC3 / 阿里 HMAC-SHA1 / 火山 SigV4 四层派生若仅用「本实现算一次、断言等于自己输出」，则签名写错也会绿 | 签名确定性测试天然有「实现即标准」的循环论证风险 | **[仅观察]** | 本版已用**独立 Python 按厂商官方文档手算**交叉核对锚定具体 hex/Base64（TC3=`cc91…decaf`、alibaba=`+uwy…32k=`、volcengine=`dac0…332a61`），并由 codex 异构裁判复核——循环论证已被外部参照打破。既有 tester「变异 sanity + 边界探测」+ producer「异构裁判」已覆盖根因，暂不机制化；若后续签名源未做独立复算即放行，再转 [晋升机制] |
| **tester/coder 撞 maxTurns**：F3-S01 / F4-S01 tester 在多源多变异（A–E）证伪处撞顶，靠续跑补完；coder 在多源接入处亦有续跑 | 多源 + 复杂签名的动态证伪/实现回合密集，超子 agent 预算 | **[仅观察]** | 与 TV1 同源、续跑兜底有效无 lost work；既有「派发切小到预算内」晋升已覆盖根因。TV2 延续观察，未升级 |
| **AWS SigV4 CanonicalHeaders 双换行疑似 bug**：火山 SigV4 规范化头末尾出现连续空行，review 时一度疑为拼接错误 | AWS SigV4 规范本就要求 CanonicalHeaders 块与 SignedHeaders 之间留一空行（blank line），非 bug | **[一次性]** | 经核对 AWS 官方 SigV4 文档确认为正确写法，reviewer 已留注释说明；属一次性认知澄清，无需机制化 |

## 晋升落地确认
- **TV2-RETRO-1 [晋升机制] 已全局落地（2026-06-06）**：项目本地 `docs/dev-log/hints.md` Hint 段（「证否密钥不泄露须用非空 sentinel 脏值」+「复杂签名独立复算锚定」）即时避坑生效；`promoted: [TV2-RETRO-1]` 已回填。**全局已落 `~/.claude/skills/code-standards/SKILL.md` §8 测试（用户批准后补落，新增 sentinel 脏值 / 独立复算锚定 / 断言验具体值不旁路三条），跨项目复用生效**。
- 往期未落地 [晋升机制]：TV1-RETRO-1 已于 TV1 落地（项目 hints.md），无遗留。
- [仅观察] 项（签名自证、tester/coder 撞顶）暂存，复发且致风险/lost work 再转 [晋升机制]。
