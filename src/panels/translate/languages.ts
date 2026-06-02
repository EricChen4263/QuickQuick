/** 单个语言选项（下拉框用）。 */
export interface LanguageOption {
  code: string;
  label: string;
}

/**
 * 源语下拉选项（含"自动检测"）。
 * 后端 source=auto/null 时回退自动检测逻辑。
 */
export const SOURCE_LANGUAGES: readonly LanguageOption[] = [
  { code: "auto", label: "自动检测" },
  { code: "zh", label: "中文" },
  { code: "en", label: "英文" },
  { code: "ja", label: "日文" },
  { code: "ko", label: "韩文" },
  { code: "fr", label: "法文" },
  { code: "de", label: "德文" },
  { code: "es", label: "西班牙文" },
  { code: "ru", label: "俄文" },
];

/**
 * 目标语下拉选项（去掉"自动检测"——目标语必须是具体语言）。
 * 从 SOURCE_LANGUAGES 过滤，保证两处定义一致。
 */
export const TARGET_LANGUAGES: readonly LanguageOption[] = SOURCE_LANGUAGES.filter(
  (l) => l.code !== "auto"
);
