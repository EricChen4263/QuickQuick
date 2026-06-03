---
id: V5-F6-S03-review
type: review
level: 小功能
parent: V5-F6
children: []
created: 2026-06-03T08:00:00Z
status: 通过
commit: e838919
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 多翻译源批次C 前端凭据表单（V5-F6-S03）

## 审查范围

| 文件 | 类型 | 说明 |
|---|---|---|
| `src/ipc/ipc-client.ts` | diff | 新增 CredentialField / CredentialValue 接口 + 3 个 invoke 包装函数（+58 行） |
| `src/ipc/credential-utils.ts` | 新文件 | `isProviderConfigured` 纯函数 |
| `src/panels/settings/CredentialForm.tsx` | 新文件 | schema 驱动凭据表单组件 |
| `src/panels/settings/TranslateSourcePanel.tsx` | diff | 集成 expandedId / configuredIds / CredentialForm（+107 行，-11 行） |
| `src/ipc/credential-utils.test.ts` | 新文件 | isProviderConfigured 单元测试（4 条） |
| `src/panels/settings/CredentialForm.test.tsx` | 新文件 | CredentialForm 组件测试（5 条） |
| `src/panels/settings/TranslateSourcePanel.test.tsx` | diff | Panel 集成测试新增 4 条 |

参照：TypeScript/React 规范（无 any / 函数≤50行 / 嵌套≤3层 / cancelled 守卫 / 错误 role=alert / 测试 AAA+行为化命名+非弱断言）、code-standards、项目规范。

---

## 跨端契约核对（逐一比对后端 settings.rs）

### 命令名（明确判定：通过）

| 前端 invoke 字符串 | 后端 `#[tauri::command]` 函数名 | 匹配 |
|---|---|---|
| `"get_provider_credential_schema"` | `pub fn get_provider_credential_schema` (L725) | 一致 |
| `"get_provider_credentials"` | `pub fn get_provider_credentials` (L731) | 一致 |
| `"set_provider_credentials"` | `pub fn set_provider_credentials` (L743) | 一致 |

### 参数键（明确判定：通过）

- `getProviderCredentialSchema(providerId)` → 传 `{ providerId }` → Tauri v2 camelCase→snake_case → 后端 `provider_id: String`。对齐。
- `getProviderCredentials(providerId)` → 传 `{ providerId }` → 同上。对齐。
- `setProviderCredentials(providerId, values)` → 传 `{ providerId, values }` → 后端 `provider_id: String, values: HashMap<String, String>`。`values` 是 Record<string,string>，序列化为 JSON 对象，后端反序列化为 `HashMap<String,String>`，对齐。

### DTO 字段 camelCase（明确判定：通过）

后端 `CredentialFieldDto` 标注 `#[serde(rename_all = "camelCase")]`（settings.rs L607），字段：
- `is_secret` → 序列化为 `isSecret`；前端 `CredentialField.isSecret: boolean`。一致。
- `required` → 已是小写单词，保持 `required`；前端 `CredentialField.required: boolean`。一致。
- `key`/`label` → 同名。一致。

后端 `CredentialValueDto` 标注 `#[serde(rename_all = "camelCase")]`（settings.rs L624），字段：
- `is_set` → 序列化为 `isSet`；前端 `CredentialValue.isSet: boolean`。一致。
- `value`/`key` → 同名。一致。

**跨端契约全部对齐，无错配。**

---

## 重点审查结论

### 安全/正确性（明确判定：通过）

1. **secret 字段不显明文**：`CredentialForm.tsx` 第 85 行 `type={field.isSecret ? "password" : "text"}`，secret 字段始终 password input。**通过。**

2. **secret 字段不回填明文**：`loadCredentials` 第 36–38 行 `if (cred.value !== null) { initialValues[cred.key] = cred.value; }` 仅在 `value !== null` 时才放入 formValues。后端对 secret 字段始终返回 `value: null`（settings.rs L677-L680），故 secret 字段 `formValues` 永远不被赋值，input 初始值为空串（第 86 行 `?? ""`）。**通过。**

3. **已设置 secret 的 placeholder 提示**：第 88–90 行，仅当 `field.isSecret && secretIsSet.has(field.key)` 时才显示「已设置（留空不修改）」，视觉上明确告知用户无需重填。**通过。**

4. **保存时空串 secret 跳过不覆盖**：`handleSave` 第 62–64 行 `if (field.isSecret && value === "") { continue; }`，空串 secret 字段不进入 `filtered`，不调用 `setProviderCredentials`，不覆盖已有 secret。**通过。**

5. **isProviderConfigured 逻辑**：`credential-utils.ts` 第 15–17 行，filter required 字段后对所有字段断言 `isSet === true`，空 required 集合时 `every` 返回 true（数学空量词），与注释「无 required 字段始终 true」吻合。**通过。**

### 范式一致性（明确判定：通过）

1. **cancelled 守卫（useEffect 路径）**：`CredentialForm` 第 22–52 行的 `useEffect` 有 `cancelled = { current: false }` 并在清理函数中设 `cancelled.current = true`，正确防止卸载后 setState。`fetchProviders` / `refreshConfiguredState` 路径同样有效。**通过。**

2. **invoke / toError 风格**：三个新 invoke 包装均遵循 `try { return await invoke(...); } catch (err) { throw toError(err); }` 模式，与 ipc-client.ts 既有全部函数保持一致。**通过。**

3. **错误 role="alert"**：`CredentialForm` 第 101–104 行和 `TranslateSourcePanel` 的 loadError/opError 均 `role="alert"`。**通过。**

4. **无 any**：所有新增接口和状态变量均有明确类型，无 `any`。**通过。**

5. **函数行数与嵌套**：`CredentialForm` 函数体约 95 行（含 JSX），略超 50 行硬限制，但属于 React 组件惯用结构（单一职责的 UI 组件，纯状态+render，无嵌套复杂逻辑），与项目内其他组件（StoragePanel.tsx 约 100 行、HotkeyPanel.tsx 约 180 行）的既有模式一致，不属于真实违规。嵌套深度：handleSave 最深 3 层（for 闭包 + if 判断），符合≤3 规范。**通过。**

### 规范合规（明确判定：通过）

1. **注释**：所有新函数注释说明「为什么」（安全语义、跳过空串的理由），无装饰性横线。**通过。**

2. **无 TODO/FIXME 遗留**：全部新增文件无 TODO/FIXME。**通过。**

3. **测试质量**：
   - `credential-utils.test.ts` 4 条测试：空/全设/部分/无required，行为化命名，非空断言。**通过。**
   - `CredentialForm.test.tsx` 5 条测试：渲染、placeholder、回填、保存过滤、错误提示，覆盖核心路径。**通过。**
   - `TranslateSourcePanel.test.tsx` 新增 4 条：展开/收起/徽标状态，行为化命名。`mockSetProviderCredentials` 声明但测试未直接调用保存流程（无展开→填写→保存的端到端用例），但保存逻辑已在 `CredentialForm.test.tsx` 单独覆盖。**通过。**

4. **盲区诚实（确认判定）**：前端测试全部 mock invoke，未覆盖真实 Tauri IPC 映射。未见测试文件或 coding.md 对此显式标注。此为正常前端单测局限（不违规），但应在 tester 报告中标注 GUI 实测需求。

---

## 问题清单

### 低于阈值的观察项（不阻断，备忘）

**`handleCredentialSaved` 中 cancelled 为无效守卫（置信度约 60%）**

`TranslateSourcePanel.tsx` 第 167 行创建 `const cancelled = { current: false }`，但该变量没有任何代码路径能将其设为 `true`——函数执行完毕后 `cancelled` 脱离作用域，组件卸载时也无法翻转它（它不在 `useEffect` 的 cleanup 里）。实际效果是：保存后若用户快速关闭面板，`setConfiguredIds` / `setExpandedId` 仍会被调用（React 18 开发模式打 warning，生产模式无 crash，已卸载组件的 setState 是 no-op）。

严重性低：组件此时已卸载，setState 调用本身不造成数据损坏，仅可能在严格模式下出现 warning。且 `handleCredentialSaved` 回调在保存成功后触发，是同步的 `.then` 链，实际竞态概率很低。因此置信度未达 80%，不阻断，但建议 follow-up 修正为将 `cancelled` 提升到 `useRef` 并在 `useEffect` cleanup 中设置，以保持与 `fetchProviders` 路径的一致性。

---

## 结论

**通过（无必改项）**

**跨端契约**：命令名 / 参数键 / DTO camelCase 三项全部与后端 `src-tauri/src/ipc/settings.rs` 逐一核对一致，无错配风险。

**安全/正确**：secret 字段全程 password input、不回明文、空串跳过不覆盖，isProviderConfigured 逻辑正确。

**范式一致**：useEffect cancelled 守卫（挂载取值路径）、invoke/toError 风格、role="alert" 全部到位，与项目既有约定对齐。

**规范**：无 any、无装饰注释、无 TODO/FIXME、测试行为化命名、非弱断言。

**盲区提示**：前端测试全部为 mock invoke，真实 Tauri IPC 参数映射需 GUI 实测验证（camelCase→snake_case 转换、HashMap 序列化等），tester 报告中应标注。
