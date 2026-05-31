---
id: V2-F1-S02-test
type: test_report
level: 小功能
parent: V2-F1
created: 2026-05-31T00:06:16Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A02]
author: tester
---

# 测试报告：V2-F1-S02 语言归一

## 1. 执行命令与结果

| # | 命令 | exit | 结论 |
|---|------|------|------|
| 1 | `cargo test --manifest-path src-tauri/Cargo.toml lang_norm` | 0 | 通过（15 passed，12 filtered out） |
| 2 | `cargo test --manifest-path src-tauri/Cargo.toml --test translate` | 0 | 通过（27 passed，0 failed） |
| 3 | `cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings` | 0 | 零警告 |

## 2. 验收用例映射表

### V2-F1-A02：lang_normalize_and_direction 套件（15 个用例全绿）

#### A02-a：detect_is_chinese——本地检测

| 验收 ID | assertion 摘要 | 测试用例 | 结果 |
|---------|----------------|---------|------|
| V2-F1-A02 | 含 CJK 字符的串判定为中文 | `lang_norm_detect_is_chinese_returns_true_for_cjk_text` | 通过 |
| V2-F1-A02 | 纯 ASCII 串不判定为中文 | `lang_norm_detect_is_chinese_returns_false_for_ascii_text` | 通过 |
| V2-F1-A02 | 纯假名（日语）不判定为中文 | `lang_norm_detect_is_chinese_returns_false_for_japanese_kana` | 通过 |
| V2-F1-A02 | 中英混合文本（中文占比≥门槛）判定为中文 | `lang_norm_detect_is_chinese_returns_true_for_mixed_text` | 通过 |
| V2-F1-A02 | 中文文本 detect_lang 返回 zh | `lang_norm_detect_lang_returns_zh_for_chinese` | 通过 |
| V2-F1-A02 | ASCII 文本 detect_lang 返回 en | `lang_norm_detect_lang_returns_en_for_ascii` | 通过 |

#### A02-b：resolve_direction——智能双向定方向

| 验收 ID | assertion 摘要 | 测试用例 | 结果 |
|---------|----------------|---------|------|
| V2-F1-A02 | 中文输入 → 目标 en（zh→en） | `lang_norm_direction_chinese_input_targets_english` | 通过 |
| V2-F1-A02 | 英文输入 → 目标 zh（en→zh） | `lang_norm_direction_english_input_targets_chinese` | 通过 |
| V2-F1-A02 | configured_target 覆盖默认方向 | `lang_norm_direction_configured_target_overrides_default` | 通过 |

#### A02-c：map_lang_for_provider——provider 映射

| 验收 ID | assertion 摘要 | 测试用例 | 结果 |
|---------|----------------|---------|------|
| V2-F1-A02 | DeepL：zh/zh-CN/zh-Hans → "ZH"（大写） | `lang_norm_deepl_maps_zh_variants_to_zh_uppercase` | 通过 |
| V2-F1-A02 | DeepL：en → "EN"（大写） | `lang_norm_deepl_maps_en_to_en_uppercase` | 通过 |
| V2-F1-A02 | MyMemory：zh/zh-Hans → "zh-CN" | `lang_norm_mymemory_maps_zh_variants_to_zh_cn` | 通过 |
| V2-F1-A02 | 百度：zh-CN/zh-Hans → "zh" | `lang_norm_baidu_maps_zh_variants_to_zh` | 通过 |
| V2-F1-A02 | Google：zh/zh-Hans → "zh-CN" | `lang_norm_google_maps_zh_variants_to_zh_cn` | 通过 |
| V2-F1-A02 | 未知 provider 原样透传（不 panic） | `lang_norm_unknown_provider_passes_through_lang_as_is` | 通过 |

**15 / 15 用例全部通过。**

## 3. translate 集成套件全量（--test translate，27 个用例）

| 套件段 | 用例数 | 通过 | 失败 |
|--------|--------|------|------|
| A02 lang_norm_*（含 mixed_text） | 15 | 15 | 0 |
| A01 provider_contract_* | 5 | 5 | 0 |
| A08 static_registry_* | 7 | 7 | 0 |
| **合计** | **27** | **27** | **0** |

S01 原有用例（A01 + A08）全绿，无回归。

## 4. 静态检查

- clippy（-D warnings）：exit=0，零警告，零错误。

## 5. 覆盖缺口

无缺口。

- `detect_is_chinese`：覆盖 CJK 正例、ASCII 反例、日语假名反例、**中英混合正例**四条路径。
- `detect_lang`：覆盖 zh 和 en 两条路径。
- `resolve_direction`：覆盖 zh→en、en→zh、configured_target 覆盖三条规则，与设计文档一一对应。
- `map_lang_for_provider`：覆盖全部 4 家 provider 的 zh 变体归一路径，以及未知 provider 透传边界。
- S01 回归（A01 + A08）：12 个用例保持绿色。

## 6. 结论

**门禁：放行。**

A02 通过（15/15 用例，含混合文本新增用例），S01 回归通过（12 用例），集成套件全量 27/27 全绿；clippy 零警告。V2-F1-S02 语言归一可进入下一任务。
