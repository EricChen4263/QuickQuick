---
id: V2-F2-S07-review
type: review
level: 小功能
parent: V2-F2
children: []
created: 2026-05-31T00:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V2-F2-A10, V2-F2-A11]
evidence: []
author: code-reviewer
---

# 代码审查报告 · V2-F2-S07（百度/DeepL/Google 适配 + 撞额度显式提示）

## 审查范围
- `src-tauri/src/translate/providers.rs`（三家完整实现 + on_quota_or_failure + 工具）+ `tests/providers.rs`（providers_keyed/quota_prompt）+ Cargo.toml(md5)
依据：code-standards（密钥不入日志/禁装饰注释）+ 设计§4.2（三家鉴权/错误归一/A11 无自动跨源铁律）。

## 通过项（核心）
**A11 铁律满足**：on_quota_or_failure 签名 `(&str,&TranslateError)->Option<UserPrompt>` 无 provider 切换参数/返回，全库 grep 无 fallback/auto_switch；密钥安全（三家 struct 未派生 Debug、无 log、secret 只参与 MD5/header/URL 参数不入 body）；无裸 unwrap/panic；thiserror；错误码 Number/String 双形态兼容（extract_number_or_string）；百度 sign=MD5(appid+q+salt+secret_key) 正确；DeepL Authorization 头；Google key 参数；needs_key 声明正确；无装饰注释；测试 AAA、5 个 A11 专项、双形态覆盖。

## 问题清单（Important，无 Critical）
**[I-01] 百度 salt 硬编码、release 也固定 → 防重放失效（置信度 85，最优先）**
- 位置：`providers.rs`（`let salt = "1435660288";` 注释称生产应随机但未分离）。
- 修复：`#[cfg(not(test))] let salt = uuid::Uuid::new_v4().simple().to_string(); #[cfg(test)] let salt = "1435660288".to_string();`（uuid 已是依赖）。

**[I-03] on_quota_or_failure 把 `{err}` 原始错误内联进用户可见 message（置信度 80）**
- 位置：`providers.rs`（`format!("...原始错误: {err}")`）。provider 原始 body（DeepL 等偶含账户邮箱/ID）随 UserPrompt 流向前端。
- 修复：去掉 `原始错误: {err}` 后缀或仅展示变体名；原始错误留日志层（日志另行过滤）。

**[I-02] DeepL 错误 body 子串匹配脆弱（置信度 80）**
- 位置：`providers.rs`（`upper.contains("AUTH")` 可能误匹配 authenticate 等）。
- 修复：parse_response 内错误 body 保守归 ServerError，精确分类交框架层 map_deepl_http_status；或精确匹配已知 DeepL 错误标记。

## 结论
**未过（打回）。** 修 I-01（cfg 随机 salt）+ I-03（裁剪用户消息原始错误）+ I-02（DeepL 错误分类精确/保守）后复审。

---

## 复审结论（2026-05-31）

**status = 通过**

- **I-01 已解决**：百度 build_request 恒用随机 salt（uuid，无 cfg/feature 分支），抽出纯函数 `baidu_sign`；`baidu-salt-fixed` feature 已从 Cargo.toml+代码清除（零残留）；测试 1）baidu_sign 固定输入确定性单测 2）从 body 解析实际随机 salt 重算签名断言——**裸跑 `cargo test providers_keyed`（无 --features）全过**（首次 feature-flag 方案破坏裸跑 verify，已纠正）。
- **I-02 已解决**：DeepL 错误分类删宽泛子串（contains("AUTH")），改精确数字状态码/明确用词 + 保守 ServerError 兜底。
- **I-03 已解决**：on_quota_or_failure 的 UserPrompt.message 删 `原始错误: {err}` 后缀，仅引导文案，provider 原始 body 不流向 UI。
A11 无自动跨源铁律仍满足（UserPrompt 无切换字段）；密钥不入日志；无装饰注释/TODO；providers 32 + 全量 154 绿。
