---
id: V2-F1-S03-review
type: review
level: 小功能
parent: V2-F1
children: []
created: 2026-05-31T08:00:00Z
status: 通过
commit: WIP
acceptance_ids: [V2-F1-A03, V2-F1-A04, V2-F1-A07]
evidence: []
author: code-reviewer
---

# V2-F1-S03 代码审查报告（错误枚举/降级/超时取消）

## 审查范围
- `src-tauri/src/translate/error.rs`（map_provider_error/classify_timeout）、`retry.rs`（is_transient/retry_with_backoff/next_backoff_ms）、`cancel.rs`（InflightTracker）、`mod.rs`（TranslateError 8 变体）+ `tests/translate.rs`（A03/A04/A07）
依据：code-standards + 设计§4.1#3（同源退避重试铁律、禁自动跨源）。

## 验收项结论
- **A03**：TranslateError 8 变体(thiserror,保留 Parse 兼容)；map_provider_error 401/403→Auth、429→RateLimit、5xx→ServerError、status0→Network、quota→Quota、too_long→TooLong、unsupported→Unsupported 映射正确；classify_timeout→Network 正确。**次要风险 P3**。核心通过。
- **A04**：is_transient 分类正确（Network/RateLimit/ServerError 瞬时，余永久）；retry_with_backoff 接受纯 FnMut 不持 provider 引用、架构上排除跨源切换。**P1 阻塞 + P2**。未通过。
- **A07**：classify_timeout→Network；InflightTracker（AtomicU64,begin 单调递增,is_current 只认最新）正确；三测试真实断言、AAA、headless。通过。

## 问题清单
### Important（阻塞）
**[P1] retry_with_backoff 退避值被丢弃，实为零间隔重试（置信度 85）**
- 位置：`retry.rs`（`let _ = next_backoff_ms(attempt);`）。FnMut 签名无法把退避传回调用方，"退避"装饰化、与函数名/注释不符。
- 修复：加 `sleep_fn: impl Fn(u64)` 参数（测试传 `|_|{}` 可断言被调用且传入退避值，生产传 `|ms| std::thread::sleep(Duration::from_millis(ms))`），使退避真实生效且可测。

**[P2] 跨源 failover 铁律测试为注释占位非真实断言（置信度 80）**
- 位置：`tests/translate.rs`（`let _ = provider_id;`）。设计铁律未被可执行断言验证。
- 修复：op 闭包内追踪每次调用的 provider_id，断言全程不变（或多 provider 池场景验证 retry 不切到第二家）。

### 次要（一并修，避免引入 TODO）
**[P3] map_by_provider_code 子串 contains 误命中风险（置信度 80）**
- 位置：`error.rs`（`contains("quota")`/`contains("unsupported")` 可能误命中 quota_remaining/unsupported_format）。
- 修复：改精确匹配已知 code 集合（或词边界匹配），不留 TODO。

## 通过项
TranslateError thiserror 8 变体完整、Parse 兼容；is_transient 与设计一致；retry 架构排除跨源；next_backoff_ms 指数退避(base 500/cap 8000)无溢出；classify_timeout→Network；InflightTracker 线程安全；无裸 unwrap/panic；无装饰注释(含 tests)；A07/A03 测试真实可执行。

## 结论
**未过（打回）。** 修 P1（退避 sleep_fn 注入生效+可测）+ P2（provider_id 不变真实断言）+ P3（精确匹配）后复审。无 Critical。

---

## 复审结论（第2轮 · 2026-05-31）

**status: 通过**

- **P1 已解决**：retry_with_backoff 加 `sleep_fn: S` 参数，每次瞬时失败后真实调 `sleep_fn(next_backoff_ms(attempt))`，退避生效无吞值。
- **P2 已解决**：retry 测试用 RefCell 真实断言 provider_id 全程不变（3 次同一 id）+ sleep 退避序列 [500,1000]；永久错误用例断言 sleep 调用 0 次；删除占位。
- **P3 已解决**：map_by_provider_code 改精确集合匹配（QUOTA_CODES/TOO_LONG_CODES/UNSUPPORTED_CODES .contains），quota_remaining/unsupported_format 不误命中，补 2 条否定边界用例。
无新引入≥80 高危；A03/A04/A07 主路径完整；无 TODO。
