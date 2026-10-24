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
 * 向字符串列表追加一项：不可变（返回新数组，不修改入参），重复项去重。
 *
 * App 名单与内容正则共用同一份追加/移除逻辑——两处的交互完全一致，
 * 只有语义不同，故各自暴露具名入口而不是让调用方直接操数组。
 */
function appendUnique(list: string[], value: string): string[] {
  if (list.includes(value)) {
    return [...list];
  }
  return [...list, value];
}

/** 从字符串列表移除一项：不可变；不存在时返回原内容的副本。 */
function removeValue(list: string[], value: string): string[] {
  return list.filter((item) => item !== value);
}

/**
 * 向 App 排除名单添加一项（隶属隐私子项）。
 *
 * 不可变：返回新数组，不修改 list。重复项去重。
 */
export function addExcludedApp(list: string[], app: string): string[] {
  return appendUnique(list, app);
}

/**
 * 从 App 排除名单移除一项（隶属隐私子项）。
 *
 * 不可变：返回新数组，不修改 list。app 不存在时返回原内容的副本。
 */
export function removeExcludedApp(list: string[], app: string): string[] {
  return removeValue(list, app);
}

/**
 * 向内容忽略正则列表添加一条（隶属隐私子项）。
 *
 * 不可变 + 重复去重；不做正则语法校验——Rust 侧编译失败只让该条失效并留痕，
 * 前端在这里拒收反而会让用户没法保存半成品规则。
 */
export function addIgnorePattern(list: string[], pattern: string): string[] {
  return appendUnique(list, pattern);
}

/**
 * 从内容忽略正则列表移除一条（隶属隐私子项）。
 *
 * 不可变：返回新数组，不修改 list。
 */
export function removeIgnorePattern(list: string[], pattern: string): string[] {
  return removeValue(list, pattern);
}
