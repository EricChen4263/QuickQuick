---
id: s06-credential-reset-ux
title: 凭据清除/重置 UX（清除已存密钥 + 已配置提示）
status: done
commit: 13d7f9b
date: 2026-06-04
---

## 来由
百度翻译卡很久的根源是 keychain 残留旧错密钥、被「留空不修改」静默保留，用户无法查看/清除已存凭据、也不知道要重填覆盖（详见 s05）。本 s06 补这个 UX 缺口。

## 后端
- `src-tauri/src/translate/credential.rs`：
  - `CredStore` trait 加 `delete_secret(provider_id, field_key)`。`KeyringCredStore` impl 用 keyring 3.6.3 的 `Entry::delete_credential()`，**NoEntry 视同成功（幂等）**；`MockCredStore` impl 从 map 移除。
  - `delete_credentials(provider_id, store, conn)`：按 schema 删——secret 字段 `store.delete_secret`、非密字段 DELETE FROM provider_config（`delete_from_db` 辅助）。未知 provider→`UnknownProvider` Err；字段未存也成功（幂等）。
- `src-tauri/src/ipc/settings.rs`：`delete_provider_credentials_impl` 纯函数 + `delete_provider_credentials(app, state, provider_id)` 命令（with_db + KeyringCredStore；删成功后 `emit(PROVIDER_CONFIG_CHANGED_EVENT)`，与 set 对称→翻译页下拉/徽标自动刷新）。
- `src-tauri/src/lib.rs`：注册命令。
- `tests/ipc_translate.rs` / `tests/translate.rs`：mock 补 `delete_secret`。

## 前端
- `src/ipc/ipc-client.ts`：`deleteProviderCredentials(providerId)` → `invoke("delete_provider_credentials", { providerId })`。
- `src/panels/settings/CredentialForm.tsx`：新增 `isConfigured` prop。
  - 已配置时：顶部显示「已配置 · 重新填写下方字段将覆盖」提示 + 「清除已存密钥」按钮。
  - 清除按钮：`window.confirm` 二次确认 → `deleteProviderCredentials` → 清空本地 formValues/secretIsSet（消除「已设置」placeholder 残留）+ onSaved（父刷新徽标）；失败 role=alert。confirm=false 直接 return，不触发任何 IPC。
- `src/panels/settings/TranslateSourcePanel.tsx`：传 `isConfigured={configuredIds.has(id)}`；`handleCredentialSaved` 补 `.catch()` 保守处理（刷新失败时把该 provider 从 configuredIds 移除，回退「待配置」，isMounted 守卫）——修 reviewer 提的 Important(85)。
- `src/panels/settings/settings.css`：补 `.credential-actions`（flex 行 gap）/`.credential-configured-hint`（var(--muted) 次要文字）/`.btn-clear-cred`（默认克制边框态、hover 危险态 var(--danger) + color-mix）。**纯 token 变量，无硬编码色，双主题**。

## TDD
- 后端：delete 删 secret(store get→None)+DB(read→None)、幂等、未知 provider→Err、delete_secret 各 store。
- 前端：isConfigured=true 显示清除按钮+提示、确认后调 delete+onSaved、取消不调、未配置不显示、失败 alert。
先红后绿。

## 实跑
```
前端 pnpm test --run：434+ passed；tsc 无错
cargo test -p quickquick：347 passed；build exit 0；fmt-check/clippy 干净
```

## 门禁
- tester PASS：4 变异（不删 secret/取消仍删/未配置也显示按钮）全红复绿；幂等、不泄漏凭据、emit 时机、confirm 守卫核对。
- reviewer 通过：安全/幂等/emit/跨端命令名一致/CSS token 均通过；一条 Important(85)「handleCredentialSaved 缺 .catch()」已修。
