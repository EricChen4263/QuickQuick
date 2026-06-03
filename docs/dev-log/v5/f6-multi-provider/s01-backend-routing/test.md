---
id: s01-backend-routing
title: 后端翻译源动态路由 测试留痕
status: passed
commit: e838919
date: 2026-06-03
---

# 测试留痕：多翻译源·批次A 后端动态路由（s01）· 动态证伪

## 命中校验
- `cargo test -p quickquick` 全量：**330 passed**（含补测后），连跑无 flaky
- `build_provider` 专项：8 passed；`translate_text_impl` 专项：6 passed
- `cargo build -p quickquick`：exit 0（确认 Box<dyn TranslateProvider> 与 credential API 编译通过）

## 变异 sanity（三变异）
- **变异A（缺 key 守卫失效）**：build_provider baidu 去掉字段守卫 → 「baidu 缺字段→Err」测试变红。还原复绿。判别力存在。
- **变异C（动态路由旁路）**：translate_text_impl 无视 selected_provider 永走 mymemory → 「selected=baidu 无凭据→Err 含未配置」测试变红。还原复绿。**动态路由被测试守护**。
- **变异B（历史 provider_id 退化）—— 首轮暴露盲区**：把写历史的 provider_id 改回硬编码 "mymemory"，**原有测试全绿未变红**（旧测试只验 selected=mymemory→id==mymemory，恒等盲区）。tester 据此**打回**。

## 打回 → 补测 → 闭合
补一个非 mymemory 场景测试 `translate_text_impl_selected_deepl_free_writes_deepl_provider_id_in_history`（translate.rs #[cfg(test)]）：
- selected_provider="deepl_free" + MockCredStore 预置 auth_key + FakeExecutor 返回 deepl 合法响应 → 翻译成功 → SQL 查 translate_history 断言 `rows[0].provider_id == "deepl_free"`。
- **实证守护力**：把生产代码 provider_id 改回硬编码 "mymemory" → 新测试**变红**（`left: "mymemory" right: "deepl_free"`）；还原 → 绿。
- 全量从 329 → **330 passed**。纯补测，生产逻辑零改动。

## 安全核对
build_provider 与 translate 路径的错误消息/日志**只含字段名不含值**（"未配置 AppID/SecretKey/auth_key/api_key"）；ProviderHttpRequest（含 Authorization 头）无打印路径。**secret 不泄漏：确认**。

## 门禁结论
**PASS（补测后放行）**
- 全量 330 全绿；build/fmt/clippy 干净
- 三变异最终均有测试守护（B 盲区已补测闭合并实证红绿）
- 安全红线（secret 不入错误/日志）通过
- 工作树无残留业务代码改动

> 动态路由的「真生效」仍需 GUI 实测（批次 C 接通前端后）：切换翻译源后翻译实际走对应 provider。
