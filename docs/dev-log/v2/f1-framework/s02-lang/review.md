---
id: V2-F1-S02-review
type: review
level: 小功能
parent: V2-F1
children: []
created: 2026-05-31T06:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A02]
evidence: []
author: code-reviewer
---

# 审查报告 · V2-F1-S02 语言归一

## 审查范围
- `src-tauri/src/translate/lang.rs`（detect_is_chinese/detect_lang/resolve_direction/normalize_zh_variant/map_lang_for_provider）+ `mod.rs` + `tests/translate.rs`（A02 三组）
标准：code-standards（禁裸 unwrap/panic、禁装饰注释）+ 设计§4.1#2/§4.3。

## 通过项
CJK 区间常量化(CJK_RANGES 非魔数,四段) ✓；假名不误判 ✓；智能双向默认(中文→en/非中文→zh,configured_target 覆盖) ✓；BCP-47 归一+provider 映射(zh/zh-CN/zh-Hans→各 provider 中文码) ✓；未知 provider 透传不 panic ✓；lang.rs 无裸 unwrap/panic ✓；测试 AAA、非恒真 ✓。

## 问题清单（Important）
**[I-1] tests/translate.rs 含装饰性分隔注释（置信度 85）**
- 位置：`src-tauri/tests/translate.rs` 第 14、50、128、206 行（`// ──── … ────`，`──` 属 `───` 同类禁令；s01 遗留，本小功能触及该文件）。
- 修复：去掉装饰横线，改普通 `// 段落名` 或 mod 分组。

**[I-2] resolve_direction 重复调用 detect_is_chinese（DRY，置信度 80）**
- 位置：`lang.rs` resolve_direction（detect_lang 内已调 detect_is_chinese，又直接再调一次）。
- 修复：复用 source 检测结果：`let default_target = if source.as_str()=="zh" { Lang::new("en") } else { Lang::new("zh") };`。

**[I-3] A02 缺中英混合文本用例（置信度 80）**
- 依据：设计§4.3 明确"适合混合文本（如 'hello 你好'）"，lang.rs 注释也提及，但 A02-a 仅测纯中文/纯 ASCII。
- 修复：补 `lang_norm_detect_is_chinese_returns_true_for_mixed_text`：`detect_is_chinese("hello 你好")==true`。

## 结论
**未过（打回）。** 修复 I-1（清装饰注释）+ I-2（去重复检测）+ I-3（补混合文本用例）后复审。无 Critical。

---

## 复审结论（第2轮 · 2026-05-31）

**status = 通过**

- **I-1 已解决**：tests/translate.rs `──/═══/━━━` 无残留，节标题改普通注释。
- **I-2 已解决**：resolve_direction 复用 detect_lang 的 source、按 `source.as_str()=="zh"` 判方向，全文本只扫一次。
- **I-3 已解决**：新增 `lang_norm_detect_is_chinese_returns_true_for_mixed_text`（"hello 你好"==true），与纯 ASCII/纯假名 false 用例构成非恒真三角。
无新引入≥80 高危；A02 调用链完整；translate 套件 27 全绿。
