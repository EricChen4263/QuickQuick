---
id: s15-hotkey-live-register
title: 改热键运行时即时生效 测试留痕
status: passed
commit: ad8cc62
date: 2026-06-03
---

# 测试留痕：热键运行时即时注册（s15）· 动态证伪

## 开工 git status 快照（业务代码部分）

```
 M src-tauri/src/ipc/settings.rs
 M src-tauri/src/lib.rs
```

---

## 一、命中校验

全量套件**连跑 3 次抗 flaky**：319 passed（24 suites），三次全绿无 flaky。

新增 2 测试精确命中（N=2，非空匹配）：
- `tests::popover_label_for_history_action_returns_clip_popover ... ok`
- `tests::popover_label_for_translate_action_returns_trans_popover ... ok`

`cargo build -p quickquick`：**exit 0**（global_shortcut 的 on_shortcut/unregister 签名编译通过——本修复关键）。

---

## 二、变异 sanity（杀恒真/旁路）

- **变异A**：`popover_label_for_action` 两分支返回值对调（History→"trans-popover"）→ 2 个映射测试均变红（`FAILED. 0 passed; 2 failed`）。还原复绿。
- **变异B**：Translate 分支固定返回 "clip-popover" → translate 测试变红、history 仍绿（`1 passed; 1 failed`）。还原复绿。

两变异均有判别力，非恒真/旁路。还原后工作树与开工快照逐行一致。

---

## 三、静态推理（glue 不可单测部分）

- **接线无漂移**：`register_action_shortcut` 回调与原 `register_hotkeys` 一致——History→`trigger_popover(handle,"clip-popover")`、Translate→`"trans-popover"`。重构等价。
- **set_hotkey 流程**：读旧键（get_hotkeys_impl）→ 持久化失败 `?` 提前返回不动运行时 → `old != accelerator` 才注销 → 注册新键失败映射 String。四条逐一确认正确。
- **既有 set_hotkey_impl 测试**未被改坏，settings 模块测试全绿。

---

## 四、稳健性次序质疑——明确结论：可接受，不回交

当前「先 unregister(old) 再 register(new)」，register(new) 失败时旧键已注销→两键失效至重启。评估可接受：
1. 失败仅 OS 级异常（新键已过冲突检测，on_shortcut 正常不失败）；
2. 持久化已完成，重启 `register_hotkeys` 自动补录新键；
3. 错误已明确反馈前端（非静默失败）；
4. 反向次序同样需处理 unregister 失败，等量复杂度——当前 YAGNI。

---

## 五、GUI 实测标注（不可单测）

「改键运行时即时生效」属 Tauri global shortcut glue，**必须用户 GUI 实测**：设置面板改热键后**不重启**，直接按新键应弹对应 popover、旧键应失效。

---

## 六、门禁结论

**PASS（放行）**

- cargo test 319 passed，连跑 3 次无 flaky；cargo build exit 0
- 2 新测试真命中；变异 A/B 如期变红再复绿
- 次序稳健性评估可接受，不回交
- 工作树无残留业务代码改动

## 结束 git status --short（业务代码部分）

```
 M src-tauri/src/ipc/settings.rs
 M src-tauri/src/lib.rs
```
与开工快照一致，无残留。
