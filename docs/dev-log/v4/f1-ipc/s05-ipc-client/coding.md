# S05 编码留痕：前端 IPC 封装层

## 改动文件

| 文件 | 说明 |
|---|---|
| `src/ipc/ipc-client.ts` | 新建：12 个类型化 invoke 封装函数 + 6 个导出接口/类型 |
| `src/ipc/ipc-client.test.ts` | 新建：20 个 vitest 单元测试，覆盖正常路径与错误重抛 |
| `docs/dev-log/v4/f1-ipc/s05-ipc-client/coding.md` | 本文件 |

## 关键实现决策

### 1. 类型对齐 Rust DTO

Rust 侧所有 DTO 均标注 `#[serde(rename_all = "camelCase")]`，前端接口字段命名与之严格对应：
- `ClipItemDto.is_favorite` → `ClipItem.isFavorite`
- `ClipItemDto.last_modified_utc` → `ClipItem.lastModifiedUtc`
- `TranslateHistoryDto.source_text` → `TranslateHistoryItem.sourceText`
- `ProviderDto.needs_key` → `Provider.needsKey`
- 等等，逐字段核对

### 2. 命令名与参数名契约

invoke 第一参数为 Rust `#[tauri::command]` 函数名（snake_case），参数对象的键名与 Rust 函数签名的参数名一致（Tauri 按名匹配）：

| 函数 | 命令名 | 参数对象 |
|---|---|---|
| `listClipItems` | `list_clip_items` | 无 |
| `deleteClipItem(id)` | `delete_clip_item` | `{ id }` |
| `toggleFavoriteClip(id, favorite)` | `toggle_favorite_clip` | `{ id, favorite }` |
| `translateText(text, target?)` | `translate_text` | `{ text, target }` |
| `listTranslateHistory` | `list_translate_history` | 无 |
| `getHotkeys` | `get_hotkeys` | 无 |
| `setHotkey(action, accelerator)` | `set_hotkey` | `{ action, accelerator }` |
| `getExcludeList` | `get_exclude_list` | 无 |
| `setExcludeList(list)` | `set_exclude_list` | `{ list }` |
| `getTranslateProviders` | `get_translate_providers` | 无 |
| `getSelectedProvider` | `get_selected_provider` | 无 |
| `setSelectedProvider(id)` | `set_selected_provider` | `{ id }` |

### 3. 错误重抛策略

Tauri invoke 在 Rust 返回 `Err(String)` 时以原始字符串 reject（不是 Error 对象）。采用统一的 `toError(cause)` 辅助函数：
- 若 cause 已是 Error 实例则原样返回
- 否则 `new Error(String(cause))` 包装，保留原始字符串作为 message
- 每个封装函数用 try/catch 捕获并调用 `toError` 重抛，绝不吞错

### 4. 泛型 invoke，禁 any

全部使用 `invoke<T>(...)` 泛型调用，不使用 `any`，通过 TypeScript strict 模式验证。

## 假设 / 未决

- `translateText` 的 `target` 参数传 `undefined` 时，Tauri 序列化为 JSON `null` 或省略该键——Rust 侧 `Option<String>` 均可接受，行为符合预期。
- `lastModifiedUtc` / `createdUtc` 使用 TypeScript `number`（对应 Rust `i64`），精度在 JS 安全整数范围内（毫秒时间戳不超过 2^53）。

## TDD 过程记录

1. RED：先写 `ipc-client.test.ts`（20 个测试），运行确认因实现文件不存在而失败（exit 1）
2. GREEN：写 `ipc-client.ts` 最小实现，发现错误路径测试中连续两次 `await expect(...).rejects` 会因 mock 已耗尽而假通过，修正为单次 catch 断言
3. REFACTOR：提取 `toError` 辅助函数消除 12 个函数中重复的错误转换逻辑；最终 20/20 通过

## code-standards 自检

| 规范项 | 状态 |
|---|---|
| 2 空格缩进、无 Tab | 通过 |
| 函数 ≤ 50 行 | 通过（最长函数含注释约 15 行） |
| 嵌套 ≤ 3 层 | 通过（最深 2 层：try → await） |
| 禁 any，用 interface/type | 通过（invoke 全部带泛型） |
| camelCase 函数与变量命名 | 通过 |
| 公共 API 有 JSDoc 注释 | 通过（含参数说明） |
| 错误处理完整，不吞异常 | 通过（toError 统一重抛） |
| 无装饰性分隔注释 | 通过 |
| 无 TODO/FIXME | 通过 |
| 无魔术字符串（命令名为必要契约，非魔术值） | 通过 |
| 测试 AAA 结构 + 行为化命名（中文） | 通过 |
| 安全：无密钥入代码 | 通过 |
