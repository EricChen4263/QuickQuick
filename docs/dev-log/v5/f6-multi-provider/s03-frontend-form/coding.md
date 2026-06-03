---
id: s03-frontend-form
title: 前端 schema 驱动凭据表单
status: done
commit: e838919
date: 2026-06-03
---

## 任务
批次 C：前端接通。选中 keyed provider → 就地展开按 credential_schema 渲染的 key 表单 → 保存（密入 keychain）→ 徽标转「已配置」。对接批次 B 的 3 条凭据 IPC。

## 改动/新增文件
- `src/ipc/ipc-client.ts`（追加）：`CredentialField {key,label,isSecret,required}`、`CredentialValue {key,value:string|null,isSet}` 类型 + 3 函数：
  - `getProviderCredentialSchema(providerId)` → `invoke("get_provider_credential_schema",{providerId})`
  - `getProviderCredentials(providerId)` → `invoke("get_provider_credentials",{providerId})`
  - `setProviderCredentials(providerId, values: Record<string,string>)` → `invoke("set_provider_credentials",{providerId, values})`（values 直接传对象，对接后端 HashMap）
- `src/ipc/credential-utils.ts`（新）：纯函数 `isProviderConfigured(schema, credentials)`——所有 required 字段都 isSet→true（无 required→true）。
- `src/panels/settings/CredentialForm.tsx`（新）：schema 驱动表单。挂载取 getProviderCredentials（cancelled 守卫）；secret 用 `<input type=password>` placeholder「已设置（留空不修改）」，非密回填值；保存时**空串 secret 跳过不覆盖**，调 setProviderCredentials；saving/error 态（role=alert）。
- `src/panels/settings/TranslateSourcePanel.tsx`（改造）：新增 expandedId / configuredIds / schemaCache；挂载对每个 needsKey provider 并行取 schema+credentials 用 isProviderConfigured 算 configuredIds；选 keyed provider→展开 CredentialForm；徽标 needsKey&configured→「已配置」、否则「待配置」。
- 3 个测试文件（credential-utils / CredentialForm / TranslateSourcePanel）。

## TDD 红绿
- isProviderConfigured：RED（模块不存在）→ GREEN（4 测试）。
- CredentialForm：RED（组件不存在）→ GREEN（5 测试）。
- TranslateSourcePanel：RED（展开/已配置未实现 2 红）→ GREEN（4 测试）。

## 实跑
```
pnpm test --run: Test Files 47 passed, Tests 413 passed
pnpm tsc --noEmit: No errors
```

## 跨端契约核对（reviewer 已逐项对后端确认一致）
命令名 get_provider_credential_schema/get_provider_credentials/set_provider_credentials；参数键 providerId（Tauri camelCase→snake_case）；set 传 values 对象（后端 HashMap）；DTO isSecret/isSet（后端 serde camelCase）。

## 暗色适配修复（GUI 实测暴露，并入 s03）
GUI 实测发现 CredentialForm 的 input/保存按钮无主题类→暗色下白底。修：input 套 `.set-input`、按钮套 `.btn btn-primary`；settings.css 补 `.credential-form/.credential-field/label/.set-input/.btn` 布局规则（纯主题变量 var()，无硬编码色，label 在上 input 全宽）；测试加 `toHaveClass("set-input"/"btn")` 防回归。tester PASS（414 全绿、变异红绿）；reviewer 通过（CSS 无硬编码色）。留痕 review-theme.md。

## 交互解耦修复（GUI 实测暴露，huashu-design 变体A，并入 s03）
GUI 实测发现：点一次 provider 卡片同时做「切默认 + 展开 key 表单」两件事，语义耦合。经 huashu-design 出 3 变体原型（docs/design/translate-source-interaction.html），用户选**变体 A**。落地：
- ProviderCard 拆为左侧 `.src-select` 热区（radio 圆点+logo+名称+徽标，点击只 onSelect 设默认）+ 右侧独立 `.src-cfg-btn`「⚙配置」按钮（**仅 needsKey 渲染**，点击只 toggle 展开表单）。
- `handleSelect` 去掉 setExpandedId 耦合；新增 `handleToggleConfigure`（独立 toggle）。
- 顺手修上面那条 cancelled 无效守卫：handleCredentialSaved 改用组件级 `useRef(true)` isMounted + cleanup 置 false 守卫。
- settings.css 新增 .src-select/.src-radio(.sel)/.src-cfg-btn(.open)，纯 tokens.css 变量无硬编码色。
- tester PASS（419 全绿；3 变异：重新耦合/配置按钮空操作/gate 失效 均如期红复绿；静态核对解耦彻底）。
- reviewer 报一条 Critical（称 handleSelect line 202 残留 setExpandedId）→ **主 agent 只读交叉验证为误报驳回**：line 202 实为 `} catch {`，setExpandedId 仅在 handleToggleConfigure(208)/handleCredentialSaved(226)；tester 实跑「点选不展开」用例 PASS 佐证。reviewer 其余项（CSS 变量存在/isMounted/a11y/规范）通过。

## 已知项 / 待 GUI 实测
- **盲区**：前端测试 mock invoke，Tauri 参数名真实映射 + 后端真实加密存取 + 保存后徽标刷新流 需 **GUI 实测**：选 keyed provider→填 key→保存→徽标「已配置」→切为默认→翻译实际走该源。
- ~~[低·非阻塞] handleCredentialSaved 的 cancelled 无效守卫~~ —— **已在「交互解耦修复」中解决**：改为组件级 `useRef(true)` isMounted + cleanup 置 false 守卫。
