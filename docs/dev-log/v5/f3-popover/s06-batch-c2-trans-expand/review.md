---
id: V5-F3-S06-review
type: review
level: 小功能
parent: V5-F3
created: 2026-06-02T00:00:00Z
status: 通过
commit: 53e064b
acceptance_ids: []
author: code-reviewer
---

> 覆盖 Batch C 整体（C1 source-text/MiniTranslate/自动翻译 + C2 展开/获焦重读）。

# 审查结论 · Batch C：trans-popover 迷你翻译

## 审查维度

- 项目规范（REMAINING-TODO.md 架构决策 §6 取词降级 + §5 窗口行为）
- code-standards（禁 any、函数 ≤50 行 ≤3 层、命名、catch promise、setState、DRY）
- 重点：cancelled flag/unlisten 清理、并发 race、lastTextRef 更新时机、非空断言安全、CSS 溢出收敛、取词偏离记录

---

## 发现问题（置信度 ≥ 80）

### 高危

无。

### 中

| # | 严重度 | 文件:行 | 问题 | 建议 |
|---|--------|---------|------|------|
| M-1 | 中 | `TransPopoverApp.tsx:41` | **lastTextRef 翻译失败后不回滚** — 行 41 在 `await translateText` 之前就把 `lastTextRef.current = text`，若翻译失败（catch 分支），ref 仍保留新文本。下次同文本获焦时 `shouldRetranslate` 返回 false，不再重试，对用户表现为"翻译失败后再触发同文本永远卡在错误状态"。就本应用的实际使用频率（用户可主动再按热键重新弹窗，弹窗会重新挂载），该场景极少发生，且下一次弹窗挂载会重新运行完整流程；但若用户不关窗仅让焦点来回切换，则会触发此缺陷。改法：将 `lastTextRef.current = text` 移至 `setStatus("done")` 之后（行 47-48 之间），仅在翻译成功后才更新 ref，失败时 ref 恢复可重试语义。 |
| M-2 | 中 | `TransPopoverApp.tsx:27-53` | **并发翻译调用无互斥** — 挂载触发一次 `runTranslateFromClipboard`，若窗口在该次翻译尚未完成时再次获焦（focus 事件），`shouldRetranslate` 会因 `lastTextRef.current` 已在行 41 被提前更新而返回 false（与 M-1 联动）；但若 M-1 修复（lastTextRef 移后），则同文本不会并发；若是不同文本，两次调用共享同一 `cancelledRef`，后者不能取消前者，两次 setState 会竞争写入最终 result/status。对于 320×200 的迷你翻译窗口、网络延迟一般小于 1-2s、而用户两次 focus 间隔通常超过该值，该场景在实践中出现概率极低。**评估为中级**（非高危）：产品影响轻微（只是偶发闪错），且修复完 M-1 后并发触发的条件（相同文本）已不会并发。若团队想彻底消除：加 `translatingRef = useRef(false)` 守卫，进入 translating 阶段时置位，catch/done 两条路径清位，focus 回调检查守卫后再触发。 |

---

## 低 / 建议

| # | 严重度 | 文件:行 | 备注 |
|---|--------|---------|------|
| L-1 | 低 | `TransPopoverApp.tsx:43` | `translateText(text!)` 非空断言 — 位于 `shouldRetranslate` 已排除 null 后，逻辑上安全（shouldRetranslate 返回 true 意味着 text !== null），但 TS 类型流无法推断，依赖人工保证。建议改为显式 `if (text === null) return;` early return 在行 40 之后，让类型自然收窄，彻底消除 `!`。 |
| L-2 | 低 | `TransPopoverApp.tsx:97-107` | `handleExpand` 中 `main?.show()` 和 `main?.setFocus()` 在 `main` 为 null 时静默跳过——符合"v1 优雅降级"意图，但若主窗不存在则用户看到 popover 消失却没有主窗出现；建议加 `if (!main) { console.warn(...); return; }` 并中止后续 hide，避免窗口消失后无主窗的迷失感。当前设计主窗永远存在，故实践中不触发，低优。 |
| L-3 | 建议 | `source-text.ts:12-13` | `pickLatestText` 不检查 `kind` 字段，依赖"图片项 content 为空串"的隐式约定；若将来后端返回带 content 的图片项（如 alt-text），会错误取走图片项文字。建议显式 `if (items[0].kind !== "text") return null;` 防御，使意图与测试注释一致（测试注释已写"图片项（content 空）"而非"kind 非 text"）。当前 ipc-client.ts 接口 kind 为 string，非 union，改为 `"text" | "image"` 枚举后此检查更有价值。 |
| L-4 | 建议 | `retranslate.ts:1-6` | 文件顶部与函数级各有一段 JSDoc，内容基本重复；顶部注释已涵盖模块语义，函数 JSDoc 再详述即可，可删去顶部模块注释或合并，避免冗余。 |
| L-5 | 建议 | `trans-popover.test.tsx` | 测试行 144 只验证 `emit` 以 "route"/"translate" 被调用，但不验证调用顺序（emit → show → setFocus → hide）。当前实现顺序是 await 串行，顺序有意义；若将来被改为 Promise.all 并行则顺序断言失效。可考虑用 `vi.fn()` 的 `mockImplementation` 加 call-order 断言；属于测试质量建议，不影响现有功能。 |

---

## 各文件小结

**`source-text.ts` / `source-text.test.ts`**：纯函数，逻辑正确，测试五条覆盖主路径。见 L-3 建议。

**`retranslate.ts` / `retranslate.test.ts`**：纯函数，四测覆盖四个关键分支（相同/不同/null-new/null-last），实现与测试一致。无问题。见 L-4 冗注建议。

**`MiniTranslate.tsx`**：纯展示组件，无副作用，props 全类型化，无 any，三按钮均有 `aria-label` 和 `type="button"`，可访问性合规。无问题。

**`TransPopoverApp.tsx`**：整体结构清晰，useEffect cleanup 路径正确（cancelled flag + unlisten?.()），focus listen 的 Promise then/catch 路径处理完整（含 resolve 到来时已 unmount 的情况），status 五态切换无遗漏，`result!` 仅在 `status === "done"` 分支使用（类型安全），函数长度合规（最长 runTranslateFromClipboard 27 行），无 any。待修复 M-1（lastTextRef 翻译失败不回滚）；M-2 为可接受风险。

**`popover.css`**：全部使用 token（`var(--fg)` / `var(--muted)` / `var(--accent)` / `var(--danger)` / `var(--r-sm)` 等），无硬编码色值；`.mini-body { flex: 1; overflow-y: auto }` 确保译文长时可滚动不溢出；与 `#root` 毛玻璃壳协调（`flex-direction: column` 链路完整）。无问题。

**取词偏离记录**：C1 coding.md（s05）已明确记载"前端挂载后直接读 listClipItems()[0]，替代原定 Rust emit 事件方案——避免竞态，零 Rust 改动"，偏离有正式记录。功能影响评估：前端读取在 `listClipItems` IPC 调用完成时，彼时剪贴板已稳定（用户先 Cmd+C 再 ⌘⇧T，序列保证），读到旧内容的风险极低；用户操作与原架构设想基本等价，降级合理。

---

## 是否合规

**合规，审查通过。**

存在 1 项逻辑缺陷 M-1（lastTextRef 在翻译失败时不回滚，相同文本失败后不可重试）和 1 项低概率并发风险 M-2；M-1 建议下一批次修复，M-2 可接受或留后续加守卫。其余规范项（禁 any、命名、函数长度、注释、DRY、CSS token、可访问性）全部合规。

**结论：status = 通过**（有 M-1 建议修复项，不阻塞 Batch D 推进，但应在版本完成前修复）。
