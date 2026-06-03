---
id: s05-baidu-sign-trim
title: 百度翻译签名错误修复 测试留痕
status: passed
commit: 2189d6a
date: 2026-06-03
---

# 测试留痕：百度签名错误修复（s05）· 动态证伪

## 命中校验
- `cargo test -p quickquick`：**338 passed**；`cargo build -p quickquick` exit 0
- 新增两测试命中：`build_provider_baidu_trims_whitespace_in_credentials`、`build_provider_baidu_whitespace_only_app_id_returns_err`

## 变异 sanity（两变异，均如期红→复绿）
- 变异A（移除 `.map(v.trim())`）：「trims_whitespace」测试变红（body 出现 %20）→ 还原复绿。
- 变异B（移除 `.filter(!empty)`）：「全空白 app_id→Err」测试变红（全空白被当有效值）→ 还原复绿。
- /tmp 备份还原，git status 与开工一致。

## 静态核对
- trim 覆盖所有 provider 所有字段（find 闭包唯一入口）。
- trim 后空字符串走 ok_or_else Err，不构造无效 provider；mymemory 可选 email 全空白→None 走无 email 路径。
- secret_key 只进 baidu_sign 的 MD5 输入、不进 body 明文；无 println/eprintln 打印凭据值。

## 门禁结论
**PASS（放行）**
- 全量 338 全绿、build exit 0
- 两变异均有测试守护、红复绿如期
- trim 覆盖完整、空=缺失正确、无凭据泄漏
- 工作树无残留业务代码改动

> 若 GUI 重试仍 54001，则密钥本身错/与 appid 不匹配（非空白），需重新复制——非代码问题。
