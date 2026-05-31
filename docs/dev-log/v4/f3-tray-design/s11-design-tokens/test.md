# V4/F3/S11 设计语言 Token — Phase 6 测试报告

验收标准：A12 `pnpm test design-tokens`
测试日期：2026-05-31
测试者：tester agent（动态证伪）

---

## 1. 命中校验

命令：`pnpm test design-tokens --run`

结果：
- Test Files: 1 passed (1)
- Tests: 12 passed (12)
- 无 0 passed 假绿，N=12 满足验收要求

结论：命中校验通过。

---

## 2. 变异 sanity

备份方式：`cp design-tokens.ts /tmp/design-tokens.ts.bak`，还原：`cp /tmp/design-tokens.ts.bak design-tokens.ts`。严禁 git checkout/restore。

### 变异 #1：BRAND_FJORD_TEAL 改坏（#3A7CA5 → #000000）

改坏内容：`export const BRAND_FJORD_TEAL = "#000000"`
期望：品牌色精确断言变红
实际：
- `BRAND_FJORD_TEAL 精确等于品牌主色 #3A7CA5` — FAIL（expected '#000000' to be '#3A7CA5'）
- Tests: 1 failed | 11 passed
已从备份还原，git status 与开工一致。
结论：测试有判别力，品牌色守门有效。

### 变异 #2：RADIUS_MD 改坏（10px → 4px）

改坏内容：`export const RADIUS_MD = "4px"`
期望：圆角精确断言 + themeToCssVars --qq-radius-md 断言变红
实际：
- `RADIUS_MD 精确等于中圆角 10px` — FAIL
- `themeToCssVars(lightTheme) 产出含 --qq-radius-md 值为 10px` — FAIL
- Tests: 2 failed | 10 passed
已从备份还原，git status 与开工一致。
结论：圆角 token 和 CSS 变量输出路径均有判别力，非旁路。

### 变异 #3（选做）：themeToCssVars 注释掉 --qq-accent 输出

改坏内容：注释 `"--qq-accent": theme.accent`
期望：themeToCssVars accent 断言变红
实际：
- `themeToCssVars(lightTheme) 产出含 --qq-accent 且值为品牌色` — FAIL（expected undefined to be '#3A7CA5'）
- Tests: 1 failed | 11 passed
已从备份还原，git status 与开工一致。
结论：CSS 变量 --qq-accent 输出路径被真实测试覆盖，非恒真。

---

## 3. 边界探测

### theme.css 实文件 grep 验证

| 关键内容 | grep 行号 | 验证 |
|---|---|---|
| `#3A7CA5` | L8, L11 | 通过 |
| `10px` | L13, L14 | 通过 |
| `prefers-color-scheme: dark` | L3 注释, L37, L76 | 通过 |
| `backdrop-filter` | L68, L70, L71 | 通过 |

### themeToCssVars 对 darkTheme 边界验证（4 项全通过）

- darkTheme 产出所有 8 个 CSS 变量，无遗漏
- lightTheme 产出所有 8 个 CSS 变量，无遗漏
- darkTheme `--qq-accent` 为合法 hex 格式（#5B9FC4）
- lightTheme 与 darkTheme 的 `--qq-bg` 不同（明暗有别）

---

## 4. 失败项 / 覆盖缺口

无失败项，无覆盖缺口。

---

## 5. 工作树完整性

开工快照：
```
?? docs/dev-log/v4/f3-tray-design/s11-design-tokens/
?? src/theme/
```
结束快照：
```
?? docs/dev-log/v4/f3-tray-design/s11-design-tokens/
?? src/theme/
```
逐行一致，变异过程中所有临时改动均已从备份还原。

---

## 门禁结论

**放行。**

三档验证全部通过：
1. 命中校验：12 passed，Test Files 1 passed，无假绿
2. 变异 sanity：3 处改坏均如期变红，测试有判别力、非恒真、非旁路
3. 边界探测：theme.css 四项关键内容实文件 grep 确认存在；darkTheme 完整变量集覆盖验证通过

