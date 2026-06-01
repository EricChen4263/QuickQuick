---
id: V5-F2-S02-coding
type: coding
phase: 5-implement
sprint: 批次A · 跨页/设置通用展示组件
created: 2026-06-01
status: done
tests: 201 passed / 0 failed
---

# 编码留痕 · 批次A：跨页/设置通用展示组件

## 范围

里程碑2 基础设施批次，为后续三页重塑（批次B/C/D）提供可复用展示组件与配套 CSS。

## 文件变更

| 文件 | 操作 | 说明 |
|---|---|---|
| `src/components/EmptyState.tsx` | 确认（已存在） | 空态组件，props: icon/title/description，渲染 .empty 结构 |
| `src/components/EmptyState.test.tsx` | 确认（已存在） | 4 个测试覆盖 DOM 结构 |
| `src/panels/settings/PanelHeader.tsx` | 确认（已存在） | .set-h + .set-sub 标题区 |
| `src/panels/settings/PanelHeader.test.tsx` | 确认（已存在） | 2 个测试 |
| `src/panels/settings/SettingGroup.tsx` | 确认（已存在） | .set-group 容器，包裹 SettingRow |
| `src/panels/settings/SettingGroup.test.tsx` | 确认（已存在） | 2 个测试 |
| `src/panels/settings/SettingRow.tsx` | 确认（已存在） | .set-row（.grow/.label/可选 .desc + 右侧 children） |
| `src/panels/settings/SettingRow.test.tsx` | 确认（已存在） | 5 个测试覆盖含/不含 description 情况 |
| `src/panels/settings/SettingToggle.tsx` | 确认（已存在） | 复用 SettingRow，右侧 button.switch[role=switch] |
| `src/panels/settings/SettingToggle.test.tsx` | **新增 2 个测试** | 补充"无 input 元素"和"type=button"断言，锁定 button 版实现 |
| `src/theme/components.css` | **修改** | 1. .switch 从旧 input+label 版改成 button::after 版；2. 末尾追加 .empty 及 .set-* 样式 |

## TDD 关键步骤

### RED → GREEN（新增测试锁定约束）

SettingToggle 已实现为 `button.switch[role=switch]`，但测试中缺少对"无 input 元素"和"type=button"的显式断言。补上两个测试后直接 GREEN，锁定已有正确行为防止未来回退：

```
it("不渲染 input 元素（button 版 switch，非旧 input+label 版）")
it("button.switch 的 type 为 button（防止表单提交）")
```

### .switch CSS 修改

旧版（里程碑1 预留）是 `input + .switch-track + .switch-thumb` 三层结构。按设计稿改为：

- 单个 `button[role=switch]`（无子元素）
- `::after` 伪元素作圆点
- `[aria-checked="true"]` 时 `background: var(--accent)` + `::after` `translateX(16px)`

里程碑1 没有任何组件使用 .switch，改动不破坏任何现有功能。

### CSS 追加

在 `components.css` 末尾追加（对照设计稿 index.html 精确还原）：
- `.empty` / `.empty .mark` / `.empty .mark svg` / `.empty h3` / `.empty p`
- `.set-h` / `.set-sub` / `.set-group` / `.set-row` / `.set-row:last-child` / `.set-row .grow` / `.set-row .label` / `.set-row .desc`

## 关键决策

1. **SettingToggle 保留 button 版不改实现**：组件已符合设计稿，只补测试约束。
2. **.switch CSS 完全替换**：旧版 CSS 与现有 button 组件不匹配（`input:checked`选择器对 button 无效），视觉上 toggle 颜色/动画完全失效，必须对齐。
3. **`.set-*` 进 components.css**：任务说明明确"设置页跨面板复用的结构类进 components.css 合理"，不单独建 settings.css（批次D 再建）。
4. **tsc 错误非本批次引入**：改动前后 tsc 错误数均为 28，diff 为空，全部为预先存在的 vitest globals 类型声明缺失（`globals: true` 在 vite 配置里，但 tsconfig `types` 未添加 vitest），不在本批次修复范围内。

## 验收门实跑结果

### 测试全量

```
npx vitest run
PASS (201) FAIL (0)
```

基准 199 → 201（新增 SettingToggle 2 个约束性测试）。

### tsc 类型检查

```
npx tsc --noEmit
exit: 2   (28 errors, 全部为预先存在的 vitest globals 缺失，本批次前后 diff 为空)
```

### 自检

- 装饰性分隔注释：无命中（src + tests）
- any 用法：无
- TODO/FIXME：无
- 断言非恒真：所有断言验具体值/具体元素
