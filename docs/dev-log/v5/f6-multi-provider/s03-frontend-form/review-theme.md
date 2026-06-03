---
id: V5-F6-S03-theme-review
type: review
level: 小功能
parent: V5-F6-S03
children: []
created: 2026-06-03T00:00:00Z
status: 通过
commit: e838919
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 凭据表单暗色样式修复（V5-F6-S03-theme）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src/panels/settings/CredentialForm.tsx` | diff | input 加 `.set-input` 类，button 加 `.btn .btn-primary` 类 |
| `src/panels/settings/settings.css` | diff | 新增 `.credential-form` / `.credential-field` / `.credential-field label` / `.credential-form .set-input` / `.credential-form .btn` 规则 |
| `src/panels/settings/CredentialForm.test.tsx` | diff | 新增类断言用例（`toHaveClass`） |

---

## CSS 无硬编码色核查

逐行核对 settings.css 新增块（第 282–312 行）：

- `.credential-form`：仅布局属性（display / flex-direction / gap / padding），无颜色值。
- `.credential-field`：仅布局属性，无颜色值。
- `.credential-field label`：`color: var(--fg)`，使用 token。
- `.credential-form .set-input`：仅 `width: 100%; box-sizing: border-box;`，颜色继承自 `.set-input` 基定义（settings.css 第 150–157 行，全部 var(--border) / var(--surface) / var(--fg)）。
- `.credential-form .btn`：仅布局属性（align-self / margin-top），颜色继承自 components.css 的 `.btn` / `.btn-primary`（var(--border) / var(--surface) / var(--fg) / var(--accent)）。

**判定：新增规则内零硬编码颜色。通过。**

`.btn-primary { color: white }` 在 components.css 是既有设计约定（primary 按钮白字，跨主题有足够对比度），不属于本次改动，不纳入判定范围。

---

## 重点核查结论

### 暗色适配修复（明确判定：通过）

- `.set-input` 使用 `var(--surface)` 背景 + `var(--fg)` 文字 + `var(--border)` 边框，暗色主题下由 token 正确驱动，原无类时浏览器默认 white 背景失效问题消除。
- `.btn.btn-primary` 使用 `var(--accent)` 背景，暗色下 accent token 已适配，修复有效。

### 与设置页其他面板一致性（明确判定：通过）

`.set-input` 是设置页统一输入框类，与热键面板、隐私面板复用同一定义。`.credential-form .set-input { width: 100% }` 仅覆盖宽度（表单场景全宽合理），未引入独立颜色定义，视觉一致。

### 间距魔术值（明确判定：通过）

新增规则间距均为 px 单位（gap: 12px / 5px，padding: 14px 16px，margin-top: 4px），符合任务约定「间距 px 可接受，颜色必须 var」。

### 测试断言有效性（明确判定：通过）

`toHaveClass("set-input")` / `toHaveClass("btn")` 为 jest-dom 精确类存在断言，非字符串子串匹配，断言有效，非弱断言。

---

## 问题清单

### 低于阈值的观察项（不阻断）

**`.btn-primary` 未纳入类断言覆盖（置信度 70%）**

`CredentialForm.test.tsx` 新增断言只验证 `toHaveClass("btn")`，未断言 `.btn-primary`。本次修复的核心目的是加 `.btn-primary` 以获得 accent 背景修复暗色白底，理论上漏掉 `btn-primary` 断言会留下回归盲区。但实现代码（CredentialForm.tsx 第 99 行）`className="btn btn-primary"` 已正确，且该类断言用例的定位是防回归，当前实现已正确。置信度 70%，不达 80% 阈值，不阻断。

---

## 结论

**通过（无必改项）**

CSS 无硬编码色：新增样式规则内零硬编码颜色，全部通过主题 token 驱动，判定通过。

暗色修复有效：`.set-input` 和 `.btn-primary` 均为主题化类，暗色下 token 正确适配，修复路径真实有效。

规范：与设置页其他面板视觉一致，间距 px 可接受，测试断言有效（非弱断言）。
