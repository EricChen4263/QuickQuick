---
status: done
category: implementation
subject: 翻译交互 P1 · trans-popover 定位到选区附近（AX 选区边界 → 光标位置回退）
summary: 热键唤起 trans-popover 时，浮窗改为贴近选中文字：在 P0 的同一次 AX 读取里追加 AXSelectedTextRange + AXBoundsForRange 取选区边界（逻辑点），新增纯函数 find_monitor_for_rect / compute_position_near_rect 在逻辑点空间计算（显示器边界由物理除回逻辑、窗口用逻辑尺寸）后贴在选区下方居中、下方放不下则上翻、两轴夹紧到所在显示器；取不到边界或边界不落在任何显示器时回退「光标所在屏居中靠上」。定位出口统一为 LogicalPosition——审查确认 tao 的 set_outer_position 会先用**窗口自身**（可能滞后的）scale 把位置转逻辑点，交物理坐标会在 1x/2x 混合布局落错屏。SelectionOutcome 保持不变，新增 SelectionSnapshot{outcome, bounds} 承载边界，P0 的 classify 与其 6 条单测原样通过。实跑（审查修复后）：cargo test --lib 591 passed / 0 failed、cargo clippy --all-targets -D warnings 退出 0、cargo fmt --check 退出 0、npx vitest run src/trans-popover src/ipc 8 files / 104 passed、npx tsc -b 退出 0。真机（浮窗与选区贴合度、混合缩放多屏落点、菜单栏/Dock 遮挡）未验，已写入 pending-manual.yaml（TP1-M01..M02）。
evidence:
  commits: []
  files:
    - "src-tauri/src/selection.rs（新增 ScreenRect / SelectionSnapshot / usable_bounds；macOS 同元素读 AXSelectedTextRange + AXBoundsForRange）"
    - "src-tauri/src/window_pos.rs（新增 MonitorSpec / find_monitor_for_rect / compute_position_near_rect + 8 条单测）"
    - "src-tauri/src/popover.rs（prepare_translate 同源带出 bounds；trigger_popover 定位优先级 bounds → 光标回退；尺寸常量单位注释订正）"
    - "docs/dev-log/pending-manual.yaml（TP1-M01..M02 真机手测项）"
  commands:
    - "cd src-tauri && cargo test --lib  # 587 passed / 0 failed"
    - "cd src-tauri && cargo clippy --all-targets -- -D warnings  # 退出 0"
    - "cd src-tauri && cargo fmt --check  # 退出 0"
    - "npx vitest run src/trans-popover src/ipc  # 8 files / 104 passed"
    - "npx tsc -b  # 退出 0"
  urls:
    - "https://developer.apple.com/documentation/applicationservices/kaxboundsforrangeparameterizedattribute"
    - "https://developer.apple.com/documentation/applicationservices/1462060-axuielementcopyparameterizedatt"
    - "https://developer.apple.com/documentation/applicationservices/1462085-axuielementcopyattributevalue"
---

# AI 活动记录 · 翻译交互 P1（浮窗贴近选区定位）

## 摘要

- 取边界与取词是**同一次** AX 会话：先取焦点应用并比对 pid，再在同一个焦点元素上读
  `AXSelectedText` → `AXSelectedTextRange` → `AXBoundsForRange`。不另起一次跨进程取词：
  show 之后源应用不再是焦点应用，第二次读只能拿到空选区。
- `SelectionOutcome` 与 `classify` 一个字节没动；新增 `SelectionSnapshot { outcome, bounds }` 承载几何，
  `usable_bounds()` 只在 outcome 为 `Text` 时才交出边界（AX 在无选中时也可能返回上次/caret 的 range）。
- 坐标换算集中在 `window_pos`：AX 给逻辑点，显示器/窗口是物理像素，按「物理 = 逻辑点 ×
  所在显示器 scale_factor」逐屏换算后与显示器物理边界对齐。
- 定位优先级：选区边界（且不与任何显示器重叠时视为不可用）→ 既有
  `compute_window_position_for_width`（光标所在屏居中靠上）。定位仍在 `show()` 之前完成。
- clip-popover 定位、翻译源、缓存/重试/取消链路、前端一律未动。

## 关键取舍

### 1. 为什么 bounds 挂在独立字段而不是塞进 `SelectionOutcome::Text`

判据（该译什么、为什么降级）与几何（浮窗放哪儿）是两件事，且几何只在读到选中文字时才有意义。
新增 `SelectionSnapshot` 后 `classify` 的签名、返回值语义与它的 6 条既有单测完全不用动，
P0 建立的分支覆盖（授权否 / 无焦点 / 空选中 / 非空选中 / 非 macOS）继续生效。

### 2. 坐标换算：全程逻辑点（审查后修订，详见文末「修复」一节）

> 初版在物理像素空间计算、再以 `PhysicalPosition` 下发；独立审查确认这在混合缩放下会落错屏，
> 已改为**全程逻辑点 + `LogicalPosition` 出口**。这里记录最终形态。

- 事实依据（读依赖源码确认，非推测）：
  - tauri 的 `WebviewWindowBuilder::inner_size` 文档写明是 **logical pixels**，窗口尺寸本来就是逻辑尺寸；
  - tao 的 `Monitor::position()` 是 `PhysicalPosition::from_logical(CGDisplayBounds.origin, scale_factor)`，
    `size()` 同理 ⇒ 显示器逻辑边界 = 物理 ÷ 本屏 scale；
  - tao 的 `set_outer_position` 是 `position.to_logical(self.scale_factor())`，而窗口隐藏时这个
    scale 还是上次所在屏的值 ⇒ 交物理坐标会被旧 scale 除错；`Logical` 变体在 `to_logical` 里
    是无损 `cast()`，不受窗口当前 scale 影响。
- 因此：AX 的 points 矩形、窗口逻辑尺寸、显示器逻辑边界在**同一个逻辑空间**里直接算，
  不再做任何乘法换算；计算用 f64，最后一次性取整成 `LogicalPosition<i32>`。
- 显示器逻辑边界用**各屏自己的** scale 派生，混合缩放（1x 主屏 + 2x 副屏）下才成立。
- 纯函数只吃逻辑量（选区矩形、窗口逻辑尺寸、带 `scale_factor` 的显示器数据），
  调用方（`popover.rs`）不需要知道任何 scale 细节。

### 3. 显示器归属按重叠面积，而不是中心点

最初实现用「选区中心点落在哪块屏」，单测立刻暴露缺陷：贴边选区（中心刚好越过屏边界几个点）
会被判成「不在任何显示器内」，白丢一份可用边界。改为取与选区**重叠面积最大**的显示器：
跨屏选区符合用户视觉归属，贴边选区也能落位。这条判据有单测
（`find_monitor_for_rect_prefers_largest_overlap`）。

### 4. `compute_position_near_rect` 返回 `Option` 而不是裸坐标

「bounds 有且落在某显示器内才用它」这条优先级因此落在返回值上，调用点写成
`geometry.and_then(...).unwrap_or_else(光标回退)` 一个表达式，而不是散在多个 if 里；
「取不到显示器」也就不用臆造一个可能跑出屏幕的坐标。

另：`scale` 由 `MonitorSpec` 携带而不是独立入参——多屏下各屏 scale 不同，单独一个 `scale`
参数会诱导调用方传错屏的值；单测通过构造 `scale_factor: 2.0` 的 `MonitorSpec` 照样覆盖换算。

### 5. 顺序：定位仍在 show 之前

沿用 P0 的时序约束注释：`show()` 之前本进程还不是前台，AX 才拿得到选区；
`set_position` 也在 `show()` 之前，面板 order front 时已经落在目标位置，没有可见跳动。

## 验证实跑（审查修复前，2026-10-31；最终数字见文末「修复」一节）

| 项 | 命令 | 结果 |
|---|---|---|
| Rust 库测试 | `cd src-tauri && cargo test --lib` | **587 passed / 0 failed**（P0 基线 575 + 本次新增 12） |
| 本次新增单测 | 同上（`selection::tests`、`window_pos::tests`） | selection 新增 4 条、window_pos 新增 8 条，全绿 |
| Rust 静态检查 | `cargo clippy --all-targets -- -D warnings` | 退出 0 |
| Rust 格式 | `cargo fmt --check` | 退出 0 |
| 前端子集 | `npx vitest run src/trans-popover src/ipc` | 8 files / **104 passed** |
| 前端类型 | `npx tsc -b` | 退出 0 |

新增单测覆盖（与 task 要求逐条对应）：

- 下方正常放置：`position_places_window_below_selection_centered`（x=390、y=328）
- 下方越界上翻：`position_flips_above_when_below_overflows`（y=792）
- 水平夹紧：`position_clamps_horizontally_to_monitor_edges`（右 1600 / 左 0）
- 选区在非主屏：`position_uses_monitor_containing_selection`、`find_monitor_for_rect_prefers_largest_overlap`
- `scale=2.0` 换算：`position_scales_logical_rect_to_physical_pixels`（580,148；未换算会得 290,78）
- 无匹配显示器回退：`position_without_matching_monitor_returns_none`
- 窗口比显示器大：`position_sticks_to_origin_when_window_wider_than_monitor`
- 边界有效性（P1 新增的几何校验）：`screen_rect_rejects_empty_and_non_finite_rects`、
  `screen_rect_keeps_valid_rect_verbatim`、`usable_bounds_*`

## 未验证项（如实说明）

- **浮窗与选区的视觉贴合度未验**：headless 环境无法 `npx tauri dev`，也无法触发全局热键；
  「AX 是否真的给出可用的 `AXBoundsForRange`」（Safari / Notes / TextEdit / Chrome / Electron 差异）
  只有真机能证，已登记 `TP1-M01`。
- **多屏 + 混合缩放 / Retina 落点未验**：换算与夹紧有单测，但真实混合缩放组合下显示器归属与
  落点需真机，已登记 `TP1-M02`。
- **非 macOS 编译未在本机交叉验证**：`bounds` 恒 `None` 的分支只在 `cfg(not(target_os = "macos"))`
  下编译，本机（macOS）跑不到；其行为等价于 P0 的非 macOS 路径（`Unsupported` + `bounds: None`）。
- **rtk 无法 spawn cargo**：本机 `rtk cargo test` 报 `Failed to spawn process: No such file or directory`
  （且返回 exit code 0，具有误导性），故全部 Rust 验证改为直跑原生 `cargo`，并在 PATH 中补入
  `$HOME/.cargo/bin`（该 bash 环境默认未包含）。
- **残余既存缺陷（不在本期范围）**：tao 的 macOS `cursor_position()` 把 `pixels_high`（物理像素）与
  鼠标点（逻辑点）混算，回退路径据此判定「光标在哪块屏」在混合缩放/上下堆叠屏上可能选错屏。
  初版遗留的「逻辑宽度减物理边界」单位不一致已在审查修复中一并解决（见文末），
  但光标来源本身的问题仍属既有缺陷（`pending-manual` H02 同源），本次未改，只把显示器归属
  留在改动前的物理空间判定上以免扩大影响面。
- **`npx vitest run src/trans-popover src/ipc` 实际为 104 passed**，而 task-2 描述写的期望是 121：
  仓库当前该子集基线即 104（P0 活动卡记录同值），本次前端零改动、数量未减少，判定为描述里的数字过期。

## 独立审查 BLOCK 后的修复（2026-10-31）

审查结论：混合缩放下落点错误（BLOCK）、极端 bounds 可能整数溢出（BLOCK）、两条文档补充。

### 问题 1 · 出口统一为逻辑点（BLOCK）

**核实**：`tao-0.35.3/src/platform_impl/macos/window.rs` 的 `set_outer_position` 是
`let scale_factor = self.scale_factor(); position.to_logical(scale_factor)`——窗口隐藏时该 scale
还是它上次所在屏的值，物理坐标会被这张旧 scale 除错、落到别的屏。而 `dpi-0.1.2` 的
`PixelUnit::to_logical` 对 `Logical` 变体是 `unit.cast()`（无损透传）。故出口必须是逻辑点。

**修法**：

- `compute_position_near_rect(rect, window_size, monitors) -> Option<LogicalPosition<i32>>`：
  全程在逻辑点空间用 f64 计算。显示器边界经 `MonitorSpec::logical_bounds()` 从物理除回逻辑
  （`物理 / 本屏 scale`），窗口尺寸直接用逻辑尺寸，**不再乘 scale**。
- 回退路径 `compute_window_position_for_width(...) -> LogicalPosition<i32>`：显示器同样取逻辑边界
  后再居中/靠上；光标仍在物理空间参与「选哪块屏」的判定（见下方取舍）。
- `popover.rs`：`translate_popover_position` 返回 `LogicalPosition<i32>`，`set_position` 收逻辑点。
- `monitor_spec/monitor_specs` 从 `popover.rs` 搬进 `window_pos.rs`（原先两处各写一份，顺手去重）。

**取舍：光标兜底里的「选哪块屏」仍留在物理空间。** tao 的 macOS `cursor_position()` 实现是
`CGDisplay::main().pixels_high()（物理像素） - point.y（逻辑点）` 再 `to_physical(主屏 scale)`，
它给出的值不是干净的逻辑点。若把它除回逻辑点再与逻辑边界比对，会改变显示器归属行为、扩大
既有缺陷的影响面。本次只改输出单位，显示器归属与改动前逐值一致；tao 那处单位混用属既有缺陷
（`pending-manual` 的 H02 观察项同源），不在本次修。

**clip-popover 的影响（明示）**：回退路径对 clip-popover 同样生效，故：

- 单屏 **1x**：与改动前逐值相同（旧实现把同样的数值当物理坐标交出，1x 下物理＝逻辑）。
  由 `center_top_position` 的 7 条既有单测（数值零改动）＋ `fallback_on_1x_matches_previous_position` 锚定。
- 单屏 **2x**：位置**有意的改变**——旧实现拿逻辑宽度 720 去减物理宽度 3840，等效落点逻辑
  `(780,162)`，比真正居中偏右 180pt（窗口宽/4）；现在为 `(600,162)`。
  由 `fallback_on_retina_returns_logical_points` 锚定。这项改变是本次 BLOCK 的同一个缺陷
  （逻辑宽度 vs 物理边界）在 clip-popover 上的表现，无法在修单位的同时保持旧值。

**新增/改写单测**（`window_pos::tests`）：

| 测试 | 锚定内容 |
|---|---|
| `position_on_retina_returns_logical_points` | 2x 屏选区 (400,50) → 逻辑 (290,78)；物理实现会得 (580,148) |
| `position_on_mixed_scale_displays_returns_target_display_logical_points` | 1x 主屏 + 2x 副屏、选区在副屏 → 逻辑 (1940,128)，且 逻辑×副屏 scale = 3880 物理 |
| `fallback_on_1x_matches_previous_position` | 1x 兜底 (600,162)，与改动前一致 |
| `fallback_on_retina_returns_logical_points` | 2x 兜底 (600,162)；旧路径等效 (780,162) |

### 问题 2 · 极端 bounds 不溢出（BLOCK）

**修法**：`compute_position_near_rect` 不再做任何 `i32` 加法——`selection_bottom + gap`、
`below + window_height` 全部在 f64 空间完成，最后一次性 `x.round() as i32`（`as` 在越界时饱和，
不 panic）。夹紧函数 `clamp_in_monitor` 对非有限值另有兜底（贴显示器起点）。
`center_top_position` 也改成 f64 中间量，避免显示器尺寸极端时整数溢出。

**单测**：`position_with_extreme_bounds_is_clamped_not_panicking`——高度 1e9 的选区，
断言不 panic 且结果为 (40,0)（被夹在屏内）。

### 问题 3 · 大选区两侧都放不下（文档）

`compute_position_near_rect` 文档写明：多行大选区上下都放不下时夹紧后**允许盖住选区的一部分**，
有意选择「浮窗完整可见」；`pending-manual` 的 TP1-M01 断言与说明同步补入该情况。

### 问题 4 · 夹紧范围含菜单栏/Dock（文档）

按审查意见不改代码，只把「夹紧范围是整块显示器、未按 visibleFrame 扣除菜单栏与 Dock」作为
已知限制写进 TP1-M01 的说明。

### 修复后实跑

| 项 | 命令 | 结果 |
|---|---|---|
| Rust 库测试 | `cd src-tauri && cargo test --lib` | **591 passed / 0 failed**（审查前 587，新增 4 条） |
| Rust 静态检查 | `cargo clippy --all-targets -- -D warnings` | 退出 0 |
| Rust 格式 | `cargo fmt --check`（仅 rustfmt 改动文件） | 退出 0 |
| 前端子集 | `npx vitest run src/trans-popover src/ipc` | 8 files / **104 passed** |
| 前端类型 | `npx tsc -b` | 退出 0 |

**仍未验证**：真机（浮窗贴合度、混合缩放多屏、菜单栏/Dock 遮挡观感）——`TP1-M01`/`TP1-M02`；
非 macOS 编译分支本机跑不到。

## 第二轮独立审查：Windows 混合 DPI 回归修复（2026-10-31）

第二轮审查给出 BLOCK（唯一阻断）：统一输出逻辑点后，Windows 上 tao 的 `set_outer_position` 用**窗口自身**的 scale 把 `to_physical` 作用于位置，而 Windows 的虚拟桌面坐标本就是物理像素。混合 DPI 下（例如主屏 150%、副屏 100%、窗口上次在主屏）逻辑点会被错误放大，落到屏外。

已核实：tao-0.35.3 `platform_impl/windows/window.rs:229-230` 为 `position.to_physical::<i32>(self.scale_factor())`；dpi 的 `PixelUnit::to_physical` 对 Physical 变体直通。

修复：
- 定位纯函数层仍全部在逻辑点空间计算（可测、与平台无关）。
- 新增平台出口 `place_for_platform(logical, target)`：macOS 交 `Position::Logical`（tao 对 Logical 透传）；其它平台交 `Position::Physical`，换算用**目标屏自身**的 scale（`target.x + 屏内逻辑偏移 × 目标屏 scale`），tao 对 Physical 直通，与窗口旧 scale 无关。
- 两条定位路径改为返回「逻辑点 + 所选目标屏」（`placement_near_rect` / `placement_for_width`），统一经出口。
- 删除已无引用的 `compute_window_position_for_width`（clippy 死代码）。
- 极端值测试：高度由 1e9 改为 1e10（旧实现会在 debug 下溢出 panic，1e9 碰不到）；新增极端宽度测试。
- 新增 `placement_on_non_macos_uses_target_monitor_scale`（非 macOS 分支，本机只能编译、不能运行该分支断言）。

审查中未采纳的项：「混合缩放 mixed-scale 测试的 1940×2==3880 恒等式」已删除；光标兜底的物理判定维持原样（显示器归属与改动前一致）。

验证（修复后实跑）：`cargo test --lib` 593 passed / 0 failed；`cargo clippy --all-targets -- -D warnings` 退出 0；`cargo fmt --check` 退出 0（仅 rustfmt 本次三个文件）；定位相关 `cargo test --lib window_pos` 21 passed。

未验证：Windows 真机（混合 DPI 落点）与非 macOS 编译分支运行，已写入 TP1-M02；clip-popover 单屏 2x 居中的真机观感同样待验。
