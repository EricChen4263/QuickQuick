# S10 托盘单一来源 — 测试报告

任务：V4/F3/S10，修复双图标缺陷（删除 tauri.conf.json app.trayIcon 块）。
执行日期：2026-05-31
测试 agent：tester (claude-sonnet-4-6)

---

## 开工 git 快照

```
 M src-tauri/tauri.conf.json
 M src-tauri/tests/boot_smoke.rs
?? docs/dev-log/v4/f3-tray-design/
```

---

## 档 1：命中校验

命令：`cargo test --manifest-path src-tauri/Cargo.toml tray_single_source`

结论：
- `test tray_single_source_no_auto_trayicon_in_conf ... ok`
- `test result: ok. 1 passed; 0 failed` — N=1，非空匹配，非假绿
- exit 0

**档 1 结论：通过**

---

## 档 2：整体编译

命令：`cargo build --manifest-path src-tauri/Cargo.toml`

结论：
- `Finished dev profile [unoptimized + debuginfo] target(s) in 0.15s`
- exit 0，无 error

**档 2 结论：通过**

---

## 档 3：变异 sanity

操作：
1. `cp src-tauri/tauri.conf.json /tmp/conf.bak` 备份
2. 用 Python json 注入 `app.trayIcon = { iconPath: "icons/32x32.png" }`（合法 JSON）
3. 跑 `cargo test tray_single_source`

变异后结果：
- `test tray_single_source_no_auto_trayicon_in_conf ... FAILED`
- `test result: FAILED. 0 passed; 1 failed` — 如期变红
- exit 101

还原：`cp /tmp/conf.bak src-tauri/tauri.conf.json`

复原验证：
- `grep trayIcon tauri.conf.json` → 无输出（正确）
- 结束 git 快照与开工逐行一致，工作树无新增/丢失

**档 3 结论：守卫有真实判别力，非恒真，变异 sanity 通过**

---

## 档 4：边界/sanity — boot_smoke 三测试

命令：`cargo test --manifest-path src-tauri/Cargo.toml --test boot_smoke`

结果：
- `test conf_json_is_valid ... ok`
- `test autostart_conf_deserializes_as_unit ... ok`
- `test tray_single_source_no_auto_trayicon_in_conf ... ok`
- `test result: ok. 3 passed; 0 failed` — 全绿
- exit 0

**档 4 结论：通过**

---

## 结束 git 快照

```
 M src-tauri/tauri.conf.json
 M src-tauri/tests/boot_smoke.rs
?? docs/dev-log/v4/f3-tray-design/
```

与开工快照逐行一致，工作树干净。

---

## 门禁结论

**放行**

所有档位通过：
- 命中校验：N=1，真命中，非假绿
- 整体编译：exit 0
- 变异 sanity：加回 trayIcon 守卫如期变红，已从备份复原，git 快照一致
- boot_smoke 三测试全绿

S10 实现质量合格，可进入下一任务。
