---
id: s04-translate-page-fixes
title: 翻译页修复（错误显示位置 + 下拉解禁已配置源 + 跨页刷新）
status: done
commit: 9dcf287
date: 2026-06-03
---

## 来由（GUI 实测暴露的两个翻译页问题）
用户填了百度 key 后：① 翻译页「翻译源」下拉里百度仍不可选；③ 翻译失败的错误显示在页面顶部、且是吞掉真实错误后的通用文案。（② 百度翻译本身报错——真实原因不明，被 ③ 吞掉，本 s04 修 ③ 让它现形，② 另行据证修。）

## ③ 错误显示：真实错误 + 移到译文区
- **根因**：`TranslatePage.handleTranslate` 的 catch 是 `} catch {`（未接 err），把后端真实错误吞成 `setError("翻译失败，请稍后重试")`；错误渲染在 TranslateWorkspace 顶部 `.tx-error`。
- **修**：
  - handleTranslate：`} catch (err) { setError(err instanceof Error ? err.message : "翻译失败，请稍后重试"); setResult(null); }`——透传后端真实错误（如「百度翻译签名错误: ...」）。
  - TranslateWorkspace：移除顶部 `.tx-error` 块；错误改在 `.tx-result` 结果区渲染（`role="alert"`，`var(--danger)`），与 result 互斥（error 优先）。

## ① 下拉解禁已配置 keyed 源 + 跨页刷新
- **根因**：`DirBar.tsx` `disabled={p.needsKey}` 对所有需 key 源无条件禁用；且 App 用 display 切页**页面不卸载**，翻译页配置状态不会在切回时刷新。
- **修**：
  - DirBar：新增 `configuredIds: Set<string>` prop（默认空 Set 向后兼容），`disabled={p.needsKey && !configuredIds.has(p.id)}`。
  - TranslateWorkspace 透传 configuredIds 给 DirBar。
  - TranslatePage：新增 configuredIds state + `fetchConfiguredIds`（对 needsKey provider 并行取 schema+credentials，复用 isProviderConfigured）；挂载计算；**监听 `PROVIDER_CONFIG_CHANGED_EVENT` 跨页刷新**（cancelled+unlisten 范式，仿 s11；事件回调用 `setProviders(curr=>{...;return curr})` 函数式读最新 providers 避 stale closure）。
  - 事件常量两端互指（仿 s10）：前端 events.ts + 后端 settings.rs；`set_provider_credentials` 命令加 `app: AppHandle`，保存成功后 `app.emit(PROVIDER_CONFIG_CHANGED_EVENT, ())`（失败仅 eprintln）。

## TDD 红绿
- ③：translate-page 测试——reject(Error("百度翻译签名错误"))→显示真实消息、错误在结果区无顶部 .tx-error。
- ①：DirBar 测试（keyed+configured 不 disabled / 未配置 disabled / 非 keyed 不 disabled）；translate-page 测试（收到事件→重算解禁）。
均先红后绿。

## 实跑
```
前端 pnpm test --run：427 passed (47 files)；tsc 无错
cargo test -p quickquick：336 passed；build exit 0；fmt-check/clippy 干净
```

## 门禁
- tester PASS：四变异（③-A 吞错误 / ③-B 错误回顶部 / ①-A 忽略 configuredIds / ①-B 不监听事件）全红复绿；事件名两端一致；真实错误透传。
- reviewer 通过：错误透传、事件名一致、无 stale closure 均 100% 置信。

## 仍待（② 另行）
- **② 百度翻译报错**：真实原因待 ③ 上线后 GUI 重试捕获真实错误消息（百度 error_code），再精准修 BaiduProvider。本 s04 不碰 provider 实现。
