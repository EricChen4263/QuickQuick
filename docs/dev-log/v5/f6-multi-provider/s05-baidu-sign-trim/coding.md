---
id: s05-baidu-sign-trim
title: 百度翻译签名错误修复（凭据 trim）
status: done
commit: 2189d6a
date: 2026-06-03
---

## 来由（GUI 实测，逐步逼近真因）
用户配置百度翻译翻译失败。经 s04 让真实错误现形后，逐步定位：
1. 先报 **52003 UNAUTHORIZED USER** → 根因：appid 只开通了「大模型文本翻译」，未开通我们调用的「通用文本翻译」(vip/translate)。用户在控制台开通通用文本翻译后此错消失。
2. 再报 **54001 Invalid Sign（签名错误）** → 本 s05 修。

## 根因（54001）
签名算法 `MD5(appid+q+salt+密钥)` 与 percent_encode（RFC 3986，按 UTF-8 字节编码中文）经核实**均正确**。appid 已被百度识别（不再 52003）但签名对不上 → 极可能是**从控制台复制 appid/密钥时带了首尾空格或换行**，参与了签名计算，而百度用干净密钥计算 → 签名不一致。

## 修复：build_provider 对凭据 trim
`src-tauri/src/translate/providers.rs` 的 `build_provider` 的 `find` 闭包加 `.map(|(_, v)| v.trim()).filter(|s| !s.is_empty())`：
- **统一 trim 所有 provider 所有字段**（baidu app_id/secret_key、deepl_free auth_key、google api_key、mymemory email）——find 闭包是字段查找唯一入口，一处覆盖全部。
- **trim 后空字符串视同字段缺失**：filter 掉 → find 返回 None → 必填字段 `ok_or_else?` 走 Err，不构造持空白凭据的 provider。
- 对**已存储**的带空白凭据**立即生效**（翻译时构造 provider 处清洗），用户无需重填。
- 下游 `BaiduProvider::new` 等无需改动。

## TDD 红绿
- `build_provider_baidu_trims_whitespace_in_credentials`：build_provider("baidu",[("app_id"," 12345 "),("secret_key"," sk ")]) → build_request body 含 `appid=12345`（trim 后、纯数字 percent_encode 不变），不含 `%20`。RED（未 trim 时 body 出现 %2012345%20）→ GREEN。
- `build_provider_baidu_whitespace_only_app_id_returns_err`：app_id 全空白 → Err（trim 后空=缺失）。RED→GREEN。

## 实跑
```
cargo test -p quickquick：338 passed；build exit 0；fmt-check/clippy 干净
```

## 门禁
- tester PASS：2 变异（移除 trim / 移除空判定）全红复绿；trim 覆盖全字段；无凭据泄漏。
- reviewer 通过：trim 后空=缺失路径正确、secret 只进 sign 不进 body 明文、54001 根因消除、无 ≥80 问题。

## 最终定位与解决（GUI 实测闭环）
trim 上线后仍报 54001。用百度官方测试向量验证我们的签名算法 `MD5(appid+q+salt+密钥)` 算出 `f89f9594663708c1605f3d736d01d2d4`，**与官方一致 → 算法证明正确**。又让用户用真实凭据直接打百度 API（GET+英文、POST+中文 两种）**均成功**，证明**密钥有效、请求构造/编码/签名全对**。再跑 KeyringCredStore 真·keychain round-trip 诊断（keyring 3.6.3 macOS 原生）：ASCII 与「中英混合+特殊字符」均**逐字节一字不差**，证明**存储层无损**。

**真正根因**：keychain 里残留了**早先输入的错密钥**——「已设置（留空不修改）」的设计使后续重开配置时密钥框为空、不重新输入就不会覆盖旧值。用户用错密钥配置过，之后没真正重填 → app 一直用错密钥算签名 → 54001。**重新粘贴正确密钥后即解决**（用户确认「没问题了」）。

- **trim 是有效的防御性改动**（防粘贴空白），保留。
- **暴露的 UX 缺口（待跟进）**：无法查看/清除已存凭据；「留空不修改」会让错值静默残留、用户无从察觉。建议补「清除/重置凭据」入口 + 更明确的已配置/重填提示（s05 未做，列为后续）。
