---
id: V5-F6-S06-review
type: review
level: 小功能
parent: V5-F6
children: []
created: 2026-06-04T00:00:00Z
status: 通过
commit: 13d7f9b
acceptance_ids: []
author: code-reviewer
---

# 审查记录 · 凭据清除/重置 UX（V5-F6-S06）

## 审查范围

| 文件 | 说明 |
|---|---|
| `src-tauri/src/translate/credential.rs` | `CredStore::delete_secret` trait 方法；`KeyringCredStore::delete_secret`（NoEntry 幂等）；`MockCredStore::delete_secret`；`delete_credentials` 函数；单测 6 条 |
| `src-tauri/src/ipc/settings.rs` | `delete_provider_credentials_impl`；`delete_provider_credentials` Tauri 命令；emit `provider-config-changed` 时机；单测 3 条（clears_all_fields / unknown_provider / idempotent） |
| `src-tauri/src/lib.rs` | 注册 `delete_provider_credentials` 命令（+1 行） |
| `src-tauri/tests/ipc_translate.rs` | `LocalMockCredStore` 补 `delete_secret` 实现 |
| `src-tauri/tests/translate.rs` | `MockCredStore`（本地）补 `delete_secret` 实现 |
| `src/ipc/ipc-client.ts` | `deleteProviderCredentials` 函数 |
| `src/panels/settings/CredentialForm.tsx` | `isConfigured` prop；已配置提示；清除按钮；`handleClear` 逻辑 |
| `src/panels/settings/TranslateSourcePanel.tsx` | 向 `CredentialForm` 传 `isConfigured` prop |
| `src/panels/settings/settings.css` | `.credential-actions` / `.btn-clear-cred` / `.credential-configured-hint` |

参照标准：Rust code-standards（安全红线 / 函数≤50行 / 嵌套≤3层 / 测试 AAA+行为化+非弱断言）、前端 TS/React 规范（禁 any / 函数式更新 / 错误处理）、项目规范。

---

## 重点检查判定

### 安全：delete 路径不打印凭据值（通过）

`KeyringCredStore::delete_secret` 只用 `keychain_account(provider_id, field_key)` 构造 account 名（格式 `cred.<pid>.<key>`），不涉及任何凭据值；错误消息仅透传 keyring 底层文本，不含 field_value。整个 delete 链路无凭据值泄漏。符合安全红线。

### 幂等性（通过）

- `KeyringCredStore::delete_secret`：匹配 `keyring::Error::NoEntry` → `Ok(())`，正确处理不存在条目。
- `delete_from_db`：执行 `DELETE FROM provider_config WHERE ...` 语义本身幂等（0 行删除不报错）。
- `delete_credentials` 按 schema 遍历，未知 provider 返回 `CredError::UnknownProvider`，已知 provider 字段不存在时两条路径均幂等成功。

有单测直接覆盖：`delete_credentials_is_idempotent_when_nothing_stored`、`delete_secret_mock_nonexistent_is_ok`。

### emit 时机（通过）

`delete_provider_credentials` 命令层在 `delete_provider_credentials_impl(...)? ` 返回 `Ok(())` 后才执行 `app.emit`，emit 失败仅记日志不影响命令返回值。与 `set_provider_credentials` 的 emit 逻辑完全对称。

### 未知 provider → Err（通过）

`delete_credentials` 开头 `if schema.is_empty() { return Err(CredError::UnknownProvider(...)) }`，有单测 `delete_credentials_unknown_provider_returns_err` 和 `delete_provider_credentials_impl_unknown_provider_returns_err` 覆盖。

### 跨端命令名/参数键一致性（通过）

前端 `invoke("delete_provider_credentials", { providerId })`；Rust 命令函数 `delete_provider_credentials(provider_id: String)`。Tauri 的 JS→Rust 参数映射规则为 camelCase→snake_case，`providerId` 自动映射 `provider_id`，一致。

### confirm=false 不触发 IPC（通过）

`handleClear` 开头 `if (!window.confirm(...)) return`，cancel 路径直接 return，不调 IPC。

### 删成功清空本地 state（通过）

`handleClear` 成功后执行 `setFormValues({})`、`setSecretIsSet(new Set())`，再调 `onSaved()` 通知父组件刷新 `configuredIds`。无残留「已设置」placeholder。

### isConfigured 控制显隐（通过）

已配置提示和清除按钮均仅在 `isConfigured && (...)` 条件下渲染；`TranslateSourcePanel` 传入 `isConfigured={configuredIds.has(provider.id)}`，来源与保存流程使用的同一 state。

### CSS 主题（通过）

`.btn-clear-cred`、`.credential-actions`、`.credential-configured-hint` 全部使用 token（`var(--border)`、`var(--muted)`、`var(--danger)`），无硬编码色值。`color-mix(in oklch, ...)` 用于 hover 背景，Safari 15.4+（macOS Monterey）已支持，Tauri2 macOS 目标无兼容问题。

---

## 问题列表

### Important · 置信度 85

**`handleCredentialSaved` Promise rejection 未处理（`TranslateSourcePanel.tsx` L212）**

```typescript
// L211-228
function handleCredentialSaved(providerId: string) {
  void getProviderCredentials(providerId).then((credentials) => {
    if (!isMounted.current) return;
    // ...
  });
  // 无 .catch(...)
}
```

清除（或保存）成功后，`handleCredentialSaved` 使用 `void promise.then(...)` 但没有 `.catch()`。若 `getProviderCredentials` 在此时 reject（IPC 瞬间失败），rejection 变为 unhandled，`configuredIds` 不更新：已清除凭据的 provider 卡片仍显「已配置」徽标，与实际状态矛盾。虽然 IPC 失败概率不高，但发生时会给用户错误反馈。

建议修复：
```typescript
function handleCredentialSaved(providerId: string) {
  void getProviderCredentials(providerId)
    .then((credentials) => {
      if (!isMounted.current) return;
      const schema = schemaCache[providerId] ?? [];
      const configured = isProviderConfigured(schema, credentials);
      setConfiguredIds((prev) => {
        const next = new Set(prev);
        if (configured) next.add(providerId); else next.delete(providerId);
        return next;
      });
      if (configured) setExpandedId(null);
    })
    .catch(() => {
      // 刷新失败时保守地从 configuredIds 移除，避免虚假「已配置」
      if (!isMounted.current) return;
      setConfiguredIds((prev) => {
        const next = new Set(prev);
        next.delete(providerId);
        return next;
      });
    });
}
```

此问题为 Important（不阻断核心清除功能，仅影响清除后的徽标刷新降级），不阻塞合并。

---

## 无 Critical 问题

置信度 ≥80 的 Critical 问题：无。

---

## 审查结论

**通过（WARNING）。**

核心功能全部合规：delete 路径安全（不泄漏凭据值）；幂等性后端 + 前端双层保障；emit 仅删除成功后触发与 set 对称；未知 provider 正确返回 Err；跨端命令名/参数键一致；confirm 二次确认保护 IPC 调用；删成功后本地 state 清空无残留；CSS 全程 token 无硬编码色。

非阻塞建议：`handleCredentialSaved` 补 `.catch()` 防止清除/保存后徽标状态在 IPC 偶发失败时残留「已配置」。

---

**VERDICT: WARNING**

`severity(Important) · confidence(85) · src/panels/settings/TranslateSourcePanel.tsx:212 · handleCredentialSaved 的 void promise.then(...) 无 .catch()，getProviderCredentials reject 时 configuredIds 不更新，清除后卡片仍显「已配置」 · 在 .then() 后追加 .catch() 保守删除 configuredIds 中对应 provider`
