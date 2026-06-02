---
id: f4-gui-fixes-s03-frontend
title: 单张图片阈值 前端接真 + L1 类型修正
status: 实现完成
commit: 6f7ab78
date: 2026-06-02
---

## 实现内容

### ipc-client.ts 新增两函数
- `getImageThreshold(): Promise<number>` — invoke `get_image_threshold`，try/catch toError。
- `setImageThreshold(bytes: number): Promise<void>` — invoke `set_image_threshold`, `{ bytes }`，try/catch toError。

### StoragePanel.tsx 接真方式
- 新增 `imageThresholdMB` state，默认 20。
- useEffect 内用 cancelled flag 防卸载后 setState，调 `getImageThreshold()` 读字节 → `Math.round(bytes / 1024 / 1024)` 存 state；失败 console.error + 保持默认 20。
- 预设档位 select（5/10/20/50/100 MB），`aria-label="单张图片阈值"`；onChange 调 `setImageThreshold(mb * 1024 * 1024)`，成功后更新 state；失败 console.error。
- desc 文案去掉「静态展示，无对应 IPC」，改为「超过此大小的图片只保留缩略图，原图标记为过大未存」。

### L1 类型修正（TranslateWorkspace.tsx）
- `onTranslate` 声明从 `() => void` 改为 `(textOverride?: string) => void`，与 TranslatePage.handleTranslate 真实签名对齐。
- 按钮 onClick 同步改为 `() => onTranslate()`，消除 MouseEvent 传入 textOverride 的 tsc 类型错误。

## 验证结果
- `pnpm test`：346 passed（41 files），含新增 6 个 StoragePanel 测试。
- `pnpm exec tsc --noEmit`：TypeScript: No errors found。
- `pnpm build`：exit 0，三入口产出，built in 349ms。
