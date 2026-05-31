export type SettingsSection =
  | "general"
  | "hotkey"
  | "translate-source"
  | "privacy"
  | "storage"
  | "about";

const SECTIONS: SettingsSection[] = [
  "general",
  "hotkey",
  "translate-source",
  "privacy",
  "storage",
  "about",
];

/** 返回设置面板六个子项，顺序固定（通用/热键/翻译源/隐私/存储/关于）。 */
export function settingsSections(): SettingsSection[] {
  return [...SECTIONS];
}

/**
 * 向 App 排除名单添加一项（隶属隐私子项）。
 *
 * 不可变：返回新数组，不修改 list。重复项去重。
 */
export function addExcludedApp(list: string[], app: string): string[] {
  if (list.includes(app)) {
    return [...list];
  }
  return [...list, app];
}

/**
 * 从 App 排除名单移除一项（隶属隐私子项）。
 *
 * 不可变：返回新数组，不修改 list。app 不存在时返回原内容的副本。
 */
export function removeExcludedApp(list: string[], app: string): string[] {
  return list.filter((item) => item !== app);
}
