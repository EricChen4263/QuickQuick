---
id: f3-s02-batch-a-frontend
title: 里程碑4 popover · Batch A2：前端脚手架
status: 实现完成
commit: PENDING
date: 2026-06-02
---

## 新建 / 改动文件

| 文件 | 说明 |
|------|------|
| `vite.config.ts` | 加 `build.rollupOptions.input` 三入口（main / clip-popover / trans-popover） |
| `src-tauri/src/popover.rs` | 两个 URL 字面量加 `src/` 前缀 |
| `src/clip-popover/index.html` | clip-popover Vite HTML 入口，含防 FOUC 主题脚本 |
| `src/clip-popover/main.tsx` | React root，接 themeStore + Esc→hide |
| `src/clip-popover/ClipPopoverApp.tsx` | 占位 UI（剪贴板标题 + 说明文字） |
| `src/clip-popover/popover.css` | 透明背景 + 毛玻璃外壳，import tokens.css |
| `src/trans-popover/index.html` | trans-popover Vite HTML 入口，含防 FOUC 主题脚本 |
| `src/trans-popover/main.tsx` | React root，接 themeStore + Esc→hide |
| `src/trans-popover/TransPopoverApp.tsx` | 占位 UI（翻译标题 + 说明文字） |
| `src/trans-popover/trans-popover.css` | 透明背景 + 毛玻璃外壳，import tokens.css |

## URL 带 `src/` 前缀的原因

Vite MPA 入口 HTML 放在 `src/clip-popover/index.html` 时，dev server 和 prod dist 的路径都是 `src/clip-popover/index.html`；Tauri `WebviewUrl::App` 在 dev/prod 均按此相对路径解析，故 popover.rs 中的 URL 必须带 `src/` 前缀才能 dev/prod 一致。

## build 结果

```
dist/index.html                          1.22 kB
dist/src/clip-popover/index.html         0.94 kB
dist/src/trans-popover/index.html        0.94 kB
dist/assets/clip-popover-B_Xn66sA.css    1.90 kB
dist/assets/clip-popover-BBw9lACL.js     0.71 kB
dist/assets/trans-popover-BYtCmB7W.js    0.71 kB
dist/assets/main-B_CsniGf.js            36.76 kB

pnpm build  → EXIT 0
pnpm test   → 31 test files, 270 tests passed
```
