---
id: RT1-F2-report
type: feature_report
level: 大功能
parent: RT1
children: [RT1-F2-S01-code, RT1-F2-S01-test, RT1-F2-S01-review, RT1-F2-S02-code, RT1-F2-S02-test, RT1-F2-S02-review, RT1-F2-S03-code, RT1-F2-S03-test, RT1-F2-S03-review]
created: 2026-06-07T00:00:00Z
status: 通过
commit: 4e0caa3
acceptance_ids: [RT1-F2-A01, RT1-F2-A02, RT1-A-SEC]
evidence: []
author: 编排（聚合）
---

# 大功能验收报告 · RT1-F2 前端富文本（渲染清洗 + 复制）

## 引用的小功能（children）
| 小功能 | 编码 | 测试 | 审查 | 状态 |
|---|---|---|---|---|
| RT1-F2-S01 渲染清洗 | [code](s01-render-sanitize/coding.md) | [test](s01-render-sanitize/test.md) | [review](s01-render-sanitize/review.md) | 通过 |
| RT1-F2-S02 复制改 IPC | [code](s02-copy-ipc/coding.md) | [test](s02-copy-ipc/test.md) | [review](s02-copy-ipc/review.md) | 通过（打回 #1 已解） |
| RT1-F2-S03 链接外部打开（RT1-M01 派生修复） | [code](s03-link-external/coding.md) | [test](s03-link-external/test.md) | [review](s03-link-external/review.md) | 通过（打回 #1 opener scope 已解） |

## 大功能级验收项对照
| 验收项 | 结果 | 证据 |
|---|---|---|
| RT1-F2-A01 富文本预览渲染（主窗口+popover）+纯文本回退 | pass | s01-render-sanitize/test.md |
| RT1-A-SEC DOMPurify 清洗，script/onerror/javascript:/iframe 四类剥离 | pass | s01-render-sanitize/test.md（去清洗变异→安全测试 RED 证判别力） |
| RT1-F2-A02 复制按钮改调 copyClipToClipboard(id) 带富文本 | pass | s02-copy-ipc/test.md（2 变异 RED） |

## 状态汇总
RT1-F2 两个小功能三联齐、客观门禁通过。S01 引入 DOMPurify 共用清洗封装、主窗口+popover 富文本渲染，安全测试覆盖四类恶意 payload（关键安全变异「去清洗」证实判别力）；S02 复制改走后端 IPC 带富文本，并收窄 kind 类型。reviewer 对 S02 打回 1 次（过时注释 hints TV1-RETRO-1 + 测试 as 断言禁 any），已修复并复审 APPROVE，未及熔断。

clippy 干净、tsc 0 错、全量 477 passed。富文本渲染靠 DOMPurify 清洗（XSS 红线，后端原样保存不清洗、CSP 未放开 script-src）。

**RT1-M01 真机验证（已确认，2026-06-07）**：暴露并修复了 2 个单测/ACL 测不到的运行时缺陷——① 渲染的 `<a href>` 点击劫持 app webview（派生 S03：tauri-plugin-opener 外部打开 + 事件委托 + scheme 白名单）；② opener `allow-open-url` 无 URL scope 致运行时 ACL 静默拒（S03 打回#1：补 `allow-default-urls`）。最终确认：富文本捕获/存储/预览/还原全保真（预览红/绿文字+链接+表格全显示）、链接外部打开、纯文本编辑器退纯文本兜底；文字颜色在备忘录/Obsidian 粘贴丢失=目标 app 接收限制（对照测试证直接浏览器→备忘录同样丢色），非缺陷。S03 为 RT1 裁决后的派生修复，不回溯改版本 done 判定。
