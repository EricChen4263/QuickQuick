// Tauri 事件名常量。
// 注意：与后端 src-tauri/src/lib.rs 的 CLIPBOARD_CHANGED_EVENT 常量必须保持一致。
// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
export const CLIPBOARD_CHANGED_EVENT = "clipboard-changed" as const;

// 翻译历史变化事件名。
// 注意：与后端 src-tauri/src/ipc/translate.rs 的 TRANSLATE_HISTORY_CHANGED_EVENT 常量必须保持一致。
// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
export const TRANSLATE_HISTORY_CHANGED_EVENT = "translate-history-changed" as const;

// provider 凭据配置变化事件名。
// 注意：与后端 src-tauri/src/ipc/settings.rs 的 PROVIDER_CONFIG_CHANGED_EVENT 常量必须保持一致。
// Tauri 事件名跨语言无法编译期共享，改动需两端同步。
export const PROVIDER_CONFIG_CHANGED_EVENT = "provider-config-changed" as const;
