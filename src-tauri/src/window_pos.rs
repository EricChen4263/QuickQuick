//! 窗口定位模块
//!
//! 两条定位策略：
//! - [`placement_for_width`]：光标所在显示器水平居中、靠上约 15%（回退路径）。
//!   取不到光标或匹配显示器时回退主显示器，再回退 (0, 0)；
//! - [`compute_position_near_rect`]：贴住选区（热键即译浮窗的主要落点，取不到选区时的回退）。
//!
//! 此模块只做坐标计算，不调用 `set_position`，便于逻辑测试。
//!
//! **单位约定：纯函数层只用逻辑点（points）；平台出口按 macOS / 其它平台区分。** 原因在 tao 的 `set_outer_position`：它先用
//! **窗口自身**的 `scale_factor()` 把 `Position` 转成逻辑点再下发，而窗口隐藏时那个 scale
//! 还是它上次所在屏的值——物理坐标在 1x/2x 混合布局里会被这张旧 scale 除错，落到别的屏上。
//! 逻辑点在 tao 侧是无损透传（`PixelUnit::Logical` 直接 `cast()`），不受窗口当前 scale 影响。
//!
//! **但这条只在 macOS 成立。** tao 的 Windows 实现同样先用窗口自身 scale 做 `to_physical`，
//! 而 Windows 的虚拟桌面坐标本来就是物理像素，逻辑点在那里会被错误放大或缩小。所以出口按平台分开：
//! macOS 交逻辑点（[`place_for_platform`] 直接返回），其它平台交物理点，并用**目标屏自己的**
//! scale 把屏内逻辑偏移换算成物理偏移。`to_physical` 对 Physical 变体是直通的，因此物理点不依赖
//! 窗口旧 scale。纯几何计算仍全部在逻辑点空间完成，只在最后一步按平台选择出口。
//!
//! 输入侧因此要分清两套单位：AX 的选区矩形（[`ScreenRect`]）本身就是逻辑点；tauri 的
//! `Monitor` 位置与尺寸是物理像素，用 [`MonitorSpec::logical_bounds`] 除回逻辑点后参与计算。

use tauri::{LogicalPosition, Position, WebviewWindow};

use crate::selection::ScreenRect;

/// 窗口顶边距离显示器顶边的比例（约 15%）。
const TOP_RATIO: f64 = 0.15;

/// 回退路径的完整放置：光标所在屏水平居中、靠上约 15%。
///
/// 返回**逻辑点坐标**与它所在的目标屏（交给 [`place_for_platform`] 做平台出口）。
/// 1x 单屏下与改动前逐值相同；2x 单屏下真正居中（改动前把逻辑宽度当物理宽度减，偏右窗口宽/4）。
pub fn placement_for_width(
    window: &WebviewWindow,
    width: i32,
) -> (LogicalPosition<i32>, Option<MonitorSpec>) {
    // 1. 取光标位置。这里刻意留在**物理**空间：tao 的 macOS `cursor_position()` 把
    //    `pixels_high`（物理像素）与鼠标点（逻辑点）混算，它给出的值并不是干净的逻辑点。
    //    沿用改动前「物理光标 vs 物理显示器边界」的判定，显示器归属与改动前逐值一致；
    //    tao 那处单位混用属于既有缺陷，不在本模块修正范围内。
    let cursor = window
        .cursor_position()
        .map(|position| (position.x, position.y))
        .unwrap_or((0.0, 0.0));

    // 2. 找到光标所在显示器（或主显示器、列表首个显示器回退）
    let monitors = monitor_specs(window);
    let target_monitor = find_monitor_at(cursor, &monitors)
        .or_else(|| {
            window
                .primary_monitor()
                .ok()
                .flatten()
                .map(|monitor| monitor_spec(&monitor))
        })
        .or_else(|| monitors.first().copied());

    // 3. 计算居中靠上位置（逻辑点）
    match target_monitor {
        Some(monitor) => (center_top_logical(monitor, width), Some(monitor)),
        None => (LogicalPosition::new(0, 0), None),
    }
}

/// 把「逻辑点」坐标交给 tauri 的最终出口：按平台选择单位。
///
/// - macOS：直接交逻辑点。tao 对 Logical 透传，不依赖窗口当前 scale。
/// - 其它平台（Windows）：交物理点。换算用的是 **`target`（浮窗将要落到的屏）自己的** scale，
///   而 tao 对 Physical 直通，因此与窗口旧 scale 无关。用目标屏而不是窗口当前所在屏，
///   是混合 DPI 下唯一正确的选择。
///
/// `logical` 必须是相对 `target` 所在逻辑空间的坐标（与 [`MonitorSpec::logical_bounds`] 同一口径）。
#[must_use]
pub fn place_for_platform(logical: LogicalPosition<i32>, target: MonitorSpec) -> Position {
    // tauri 的 Position 是 f64 泛型；i32 逻辑点在这里只是数值来源，精度不受影响。
    #[cfg(target_os = "macos")]
    {
        let _ = target;
        Position::Logical(tauri::LogicalPosition::new(
            f64::from(logical.x),
            f64::from(logical.y),
        ))
    }

    #[cfg(not(target_os = "macos"))]
    {
        // 屏内逻辑偏移 × 本屏 scale + 本屏物理原点 = 该屏物理空间里的绝对坐标。
        let scale = target.scale();
        let (left, top, _, _) = target.logical_bounds();
        let x = f64::from(target.x) + (f64::from(logical.x) - left) * scale;
        let y = f64::from(target.y) + (f64::from(logical.y) - top) * scale;
        Position::Physical(tauri::PhysicalPosition::new(
            x.round() as i32,
            y.round() as i32,
        ))
    }
}

/// 贴选区的完整放置：返回逻辑点坐标与所选屏；选区不落在任何已知屏内时返回 `None`。
#[must_use]
pub fn placement_near_rect(
    rect: ScreenRect,
    window_size: (i32, i32),
    monitors: &[MonitorSpec],
) -> Option<(LogicalPosition<i32>, MonitorSpec)> {
    let monitor = find_monitor_for_rect(rect, monitors)?;
    compute_position_near_rect(rect, window_size, monitors).map(|position| (position, monitor))
}

/// 把 tauri 的显示器句柄转成纯几何数据（物理像素边界 + 缩放比）。
///
/// 保留物理字段而不是在这里就除成逻辑点：显示器归属与外接布局的判断都基于物理几何，
/// 逻辑点只在 [`MonitorSpec::logical_bounds`] 这个出口按各屏 scale 派生。
pub fn monitor_spec(monitor: &tauri::Monitor) -> MonitorSpec {
    let position = monitor.position();
    let size = monitor.size();
    MonitorSpec {
        x: position.x,
        y: position.y,
        width: size.width as i32,
        height: size.height as i32,
        scale_factor: monitor.scale_factor(),
    }
}

/// 当前全部显示器的几何数据；取不到列表时返回空表（调用方据此降级到主显示器或光标兜底）。
pub fn monitor_specs(window: &WebviewWindow) -> Vec<MonitorSpec> {
    match window.available_monitors() {
        Ok(monitors) => monitors.iter().map(monitor_spec).collect(),
        Err(e) => {
            eprintln!("[QuickQuick] 获取显示器列表失败，回退主显示器: {e}");
            Vec::new()
        }
    }
}

/// 在给定显示器列表中查找包含物理点 `(x, y)` 的显示器。
///
/// 用物理点：调用方给的是 tao 的 `cursor_position()`，它本身不是干净的逻辑点（见调用处注释）。
fn find_monitor_at((cx, cy): (f64, f64), monitors: &[MonitorSpec]) -> Option<MonitorSpec> {
    monitors.iter().copied().find(|monitor| {
        let left = f64::from(monitor.x);
        let top = f64::from(monitor.y);
        let right = left + f64::from(monitor.width);
        let bottom = top + f64::from(monitor.height);
        cx >= left && cx < right && cy >= top && cy < bottom
    })
}

/// 在给定显示器区域内计算水平居中、靠上 TOP_RATIO 的**逻辑点**坐标。
///
/// 参数均为逻辑点：`mon_x` / `mon_y` 是显示器左上角，`mon_w` / `mon_h` 是显示器宽高，
/// `width` 是窗口逻辑宽度。
///
/// 内部退到 f64 再截断：既与改动前的整数除法逐值一致（1x 单屏行为不变），又不会在极端
/// 显示器尺寸下整数溢出。`as i32` 在越界时饱和，不 panic。
fn center_top_position(
    mon_x: i32,
    mon_y: i32,
    mon_w: i32,
    mon_h: i32,
    width: i32,
) -> LogicalPosition<i32> {
    let x = f64::from(mon_x) + (f64::from(mon_w) - f64::from(width)) / 2.0;
    let y = f64::from(mon_y) + f64::from(mon_h) * TOP_RATIO;
    LogicalPosition::new(x as i32, y as i32)
}

/// 光标兜底落点的纯计算：显示器逻辑边界 + 窗口逻辑宽度 → 逻辑坐标。
///
/// 这里不接收任何 scale：显示器物理边界先除回逻辑点，窗口尺寸本来就是逻辑尺寸，
/// 算出来的结果与「窗口当前在哪块屏、当前 scale 是几」无关——这正是本次修掉的那条耦合。
fn center_top_logical(monitor: MonitorSpec, width: i32) -> LogicalPosition<i32> {
    let (left, top, mon_width, mon_height) = monitor.logical_bounds();
    center_top_position(
        left as i32,
        top as i32,
        mon_width as i32,
        mon_height as i32,
        width,
    )
}

/// 选区与浮窗之间的垂直间距（逻辑点）：留一点缝，浮窗不压在选区下沿上。
const ANCHOR_GAP_POINTS: f64 = 8.0;

/// 一块显示器的几何信息：物理像素边界 + 缩放比。
///
/// 为什么不直接用 `tauri::Monitor`：它没有公开构造函数，测试里造不出来；把需要的字段
/// 搬进纯数据类型后，「多屏选择」「Retina 换算」这些判据才能在没有 GUI 的环境里断言到。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MonitorSpec {
    /// 显示器左上角 x（物理像素）。
    pub x: i32,
    /// 显示器左上角 y（物理像素）。
    pub y: i32,
    /// 显示器宽度（物理像素）。
    pub width: i32,
    /// 显示器高度（物理像素）。
    pub height: i32,
    /// 缩放比（Retina 为 2.0）。非法值按 1.0 处理，见 [`Self::scale`]。
    pub scale_factor: f64,
}

impl MonitorSpec {
    /// 参与换算的缩放比；非有限或非正的值退成 1.0，避免除零与 NaN 扩散到坐标里。
    fn scale(&self) -> f64 {
        if self.scale_factor.is_finite() && self.scale_factor > 0.0 {
            self.scale_factor
        } else {
            1.0
        }
    }

    /// 显示器边界的逻辑点表示。
    ///
    /// 为什么可以直接除：tauri 的显示器位置与尺寸由 `CGDisplayBounds`（点）× scale_factor
    /// 得到，因此「物理 = 点 × 本屏 scale」在全局坐标空间里逐屏成立。
    fn logical_bounds(&self) -> (f64, f64, f64, f64) {
        let scale = self.scale();
        (
            f64::from(self.x) / scale,
            f64::from(self.y) / scale,
            f64::from(self.width) / scale,
            f64::from(self.height) / scale,
        )
    }

    /// 选区与本显示器在逻辑点空间的重叠面积；无重叠为 0。
    ///
    /// 用面积而不是「中心点是否在内」：贴边选区（中心可能落在屏幕边界之外一点点）如果按
    /// 中心判，会得到「不在任何显示器内」而白白丢掉可用边界。
    fn overlap_area(&self, rect: ScreenRect) -> f64 {
        let (left, top, width, height) = self.logical_bounds();
        let overlap_width = (rect.x + rect.width).min(left + width) - rect.x.max(left);
        let overlap_height = (rect.y + rect.height).min(top + height) - rect.y.max(top);
        overlap_width.max(0.0) * overlap_height.max(0.0)
    }
}

/// 找出选区所在的显示器：取与选区重叠面积最大的那块（无重叠返回 `None`）。
///
/// 为什么不是「中心点属于哪块」：选区跨屏时两屏各有重叠，按面积判才是用户视觉上的归属；
/// 贴边选区也不会因为中心刚好越界而丢掉可用边界。
#[must_use]
pub fn find_monitor_for_rect(rect: ScreenRect, monitors: &[MonitorSpec]) -> Option<MonitorSpec> {
    let mut best: Option<(f64, MonitorSpec)> = None;
    for monitor in monitors.iter().copied() {
        let area = monitor.overlap_area(rect);
        if area <= 0.0 {
            continue;
        }
        // 面积相等时保留先命中的那块：显示器列表顺序稳定，结果才可复现。
        let is_better = match best {
            Some((best_area, _)) => area > best_area,
            None => true,
        };
        if is_better {
            best = Some((area, monitor));
        }
    }
    best.map(|(_, monitor)| monitor)
}

/// 计算「贴近选区」的窗口**逻辑点**坐标。
///
/// 摆放规则：
/// 1. 水平：以选区中心居中，再夹紧到该显示器内；
/// 2. 垂直：贴在选区下沿之下 `ANCHOR_GAP_POINTS`；下方放不下就翻到选区上方；
/// 3. 两条轴最终都夹紧到该显示器内，避免浮窗被推到屏幕外。
///
/// 已知取舍：多行大选区（选区比可用高度还高）上下都放不下时，夹紧后的浮窗会**盖住选区的一部分**。
/// 这里明确选择「浮窗完整可见、允许遮挡」——被裁到屏幕外的浮窗更难用，而选区内容用户已经看到。
///
/// 全程在逻辑点空间用 f64 计算、最后一次性取整：AX 的极端 bounds（例如高度 1e9）在 f64
/// 加法里不会溢出，`as i32` 在越界时饱和，因此既不会 debug panic，结果也仍被夹紧在屏内。
///
/// 返回 `None` 表示选区不与任何已知显示器重叠（坐标空间对不上或显示器列表为空）：
/// 调用方据此回退到光标定位，「bounds 有且落在某显示器内才用它」这条优先级因此落在
/// 返回值上，而不是散在调用方的 if 里。
#[must_use]
pub fn compute_position_near_rect(
    rect: ScreenRect,
    window_size: (i32, i32),
    monitors: &[MonitorSpec],
) -> Option<LogicalPosition<i32>> {
    let monitor = find_monitor_for_rect(rect, monitors)?;
    let (left, top, mon_width, mon_height) = monitor.logical_bounds();
    let window_width = f64::from(window_size.0);
    let window_height = f64::from(window_size.1);

    let centered_x = rect.x + rect.width / 2.0 - window_width / 2.0;
    let x = clamp_in_monitor(centered_x, left, mon_width, window_width);

    let below = rect.y + rect.height + ANCHOR_GAP_POINTS;
    let above = rect.y - ANCHOR_GAP_POINTS - window_height;
    let fits_below = below + window_height <= top + mon_height;
    let y = clamp_in_monitor(
        if fits_below { below } else { above },
        top,
        mon_height,
        window_height,
    );

    // 取整到最近的逻辑点；`as i32` 越界饱和，不会 panic。
    Some(LogicalPosition::new(x.round() as i32, y.round() as i32))
}

/// 把窗口起点夹紧到显示器逻辑区间内；窗口比显示器还大时贴住显示器起点。
fn clamp_in_monitor(value: f64, origin: f64, extent: f64, window_length: f64) -> f64 {
    // 上限不小于下限，clamp 才不会 panic：窗口比显示器大时上下限都退成 origin。
    let max = origin + (extent - window_length).max(0.0);
    if value.is_finite() {
        value.clamp(origin, max)
    } else {
        // NaN 无从比较、±inf 无法落位：贴住显示器起点，交出确定的可见位置。
        origin
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证水平居中计算：显示器 1920×1080，窗口 400px 宽，x 应为 760
    #[test]
    fn center_top_x_is_centered() {
        // Arrange: 显示器起点 (0,0)，大小 1920×1080，窗口宽 400
        // Act
        let pos = center_top_position(0, 0, 1920, 1080, 400);

        // Assert: x = (1920 - 400) / 2 = 760
        assert_eq!(pos.x, 760, "水平居中 x 应为 760");
    }

    /// 验证靠上 15%：1080 * 0.15 = 162
    #[test]
    fn center_top_y_is_fifteen_percent() {
        // Arrange
        // Act
        let pos = center_top_position(0, 0, 1920, 1080, 400);

        // Assert: y = floor(1080 * 0.15) = 162
        assert_eq!(pos.y, 162, "靠上 15% y 应为 162");
    }

    /// 验证多显示器偏移：显示器起点 (1920, 200)，大小 2560×1440
    #[test]
    fn center_top_accounts_for_monitor_offset() {
        // Arrange
        // Act
        let pos = center_top_position(1920, 200, 2560, 1440, 400);

        // Assert: x = 1920 + (2560 - 400) / 2 = 1920 + 1080 = 3000
        //         y = 200 + floor(1440 * 0.15) = 200 + 216 = 416
        assert_eq!(pos.x, 3000, "多显示器偏移 x 应为 3000");
        assert_eq!(pos.y, 416, "多显示器偏移 y 应为 416");
    }

    /// 验证 find_monitor_at：空列表返回 None
    #[test]
    fn find_monitor_at_no_monitors_returns_none() {
        // Arrange
        let monitors: Vec<MonitorSpec> = vec![];

        // Act
        let result = find_monitor_at((100.0, 100.0), &monitors);

        // Assert
        assert!(result.is_none(), "空显示器列表应返回 None");
    }

    /// 验证 center_top_position 带 width 参数：1920×1080 + width=720，x 应为 600
    #[test]
    fn center_top_with_width_720_gives_correct_x() {
        // Arrange: 显示器 (0,0) 1920×1080，width=720
        // Act
        let pos = center_top_position(0, 0, 1920, 1080, 720);

        // Assert: x = (1920 - 720) / 2 = 600
        assert_eq!(pos.x, 600, "width=720 时 x 应为 600");
    }

    /// 验证 center_top_position 带 width 参数：1920×1080 + width=320，x 应为 800
    #[test]
    fn center_top_with_width_320_gives_correct_x() {
        // Arrange: 显示器 (0,0) 1920×1080，width=320
        // Act
        let pos = center_top_position(0, 0, 1920, 1080, 320);

        // Assert: x = (1920 - 320) / 2 = 800
        assert_eq!(pos.x, 800, "width=320 时 x 应为 800");
    }

    /// 验证 center_top_position 带 width 参数：y 仍然是 15%（与 width 无关）
    #[test]
    fn center_top_with_custom_width_y_unchanged() {
        // Arrange
        // Act
        let pos = center_top_position(0, 0, 1920, 1080, 720);

        // Assert: y = floor(1080 * 0.15) = 162
        assert_eq!(pos.y, 162, "y 应为 162（与 width 无关）");
    }

    /// 单屏 1920×1080 @1x，主显示器常用基准。
    fn primary_monitor() -> MonitorSpec {
        MonitorSpec {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
            scale_factor: 1.0,
        }
    }

    /// 选区右侧 2560×1440 的第二块屏，起点接在主屏右边。
    fn secondary_monitor() -> MonitorSpec {
        MonitorSpec {
            x: 1920,
            y: 0,
            width: 2560,
            height: 1440,
            scale_factor: 1.0,
        }
    }

    /// 选中文字时：浮窗在选区下方、水平居中。
    #[test]
    fn position_places_window_below_selection_centered() {
        // Arrange: 选区 (500,300) 100×20，浮窗逻辑尺寸 320×200
        let rect = ScreenRect {
            x: 500.0,
            y: 300.0,
            width: 100.0,
            height: 20.0,
        };

        // Act
        let position = compute_position_near_rect(rect, (320, 200), &[primary_monitor()]);

        // Assert: x = (500 + 50) - 160 = 390；y = (300 + 20) + 8 = 328
        assert_eq!(position, Some(LogicalPosition::new(390, 328)));
    }

    /// 下方放不下时翻到选区上方，且底边与选区上沿之间保留同样的间距。
    #[test]
    fn position_flips_above_when_below_overflows() {
        // Arrange: 选区下沿 1020，浮窗高 200 时下方已不够
        let rect = ScreenRect {
            x: 500.0,
            y: 1000.0,
            width: 100.0,
            height: 20.0,
        };

        // Act
        let position = compute_position_near_rect(rect, (320, 200), &[primary_monitor()]);

        // Assert: y = 1000 - 8 - 200 = 792
        assert_eq!(position, Some(LogicalPosition::new(390, 792)));
    }

    /// 贴边选区：水平方向夹紧到显示器内，不把浮窗推出屏幕。
    #[test]
    fn position_clamps_horizontally_to_monitor_edges() {
        // Arrange: 右边缘选区（中心 1850）与左边缘选区（中心 5）
        let right_edge = ScreenRect {
            x: 1800.0,
            y: 300.0,
            width: 100.0,
            height: 20.0,
        };
        let left_edge = ScreenRect {
            x: 0.0,
            y: 300.0,
            width: 10.0,
            height: 20.0,
        };

        // Act
        let at_right = compute_position_near_rect(right_edge, (320, 200), &[primary_monitor()]);
        let at_left = compute_position_near_rect(left_edge, (320, 200), &[primary_monitor()]);

        // Assert: 右边夹到 1920 - 320 = 1600，左边夹到 0
        assert_eq!(at_right.map(|p| p.x), Some(1600), "右边缘应夹到 1600");
        assert_eq!(at_left.map(|p| p.x), Some(0), "左边缘应夹到 0");
    }

    /// 跨屏选区按重叠面积归属：左屏占两成、右屏占八成时落在右屏。
    #[test]
    fn find_monitor_for_rect_prefers_largest_overlap() {
        // Arrange: 选区 x=1900..2100，跨主屏（至 1920）与副屏（自 1920 起）
        let spanning = ScreenRect {
            x: 1900.0,
            y: 100.0,
            width: 200.0,
            height: 20.0,
        };
        let monitors = [primary_monitor(), secondary_monitor()];

        // Act
        let monitor = find_monitor_for_rect(spanning, &monitors);

        // Assert: 主屏重叠 20pt，副屏重叠 180pt
        assert_eq!(monitor, Some(secondary_monitor()), "重叠更大的副屏胜出");
    }

    /// 选区在副屏时按副屏的边界定位，而不是主屏。
    #[test]
    fn position_uses_monitor_containing_selection() {
        // Arrange: 选区落在副屏（起点 x=1920）
        let rect = ScreenRect {
            x: 2200.0,
            y: 100.0,
            width: 200.0,
            height: 20.0,
        };
        let monitors = [primary_monitor(), secondary_monitor()];

        // Act
        let monitor = find_monitor_for_rect(rect, &monitors);
        let position = compute_position_near_rect(rect, (320, 200), &monitors);

        // Assert: 命中副屏；x = (2200 + 100) - 160 = 2140，y = (100 + 20) + 8 = 128
        assert_eq!(monitor, Some(secondary_monitor()));
        assert_eq!(position, Some(LogicalPosition::new(2140, 128)));
    }

    /// Retina 单屏：落点按**逻辑点**交出，等于「物理落点 ÷ 该屏 scale」。
    #[test]
    fn position_on_retina_returns_logical_points() {
        // Arrange: 2560×1440 物理像素的 2x 屏（逻辑 1280×720），选区 (400,50) 100×20 点
        let retina = MonitorSpec {
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
            scale_factor: 2.0,
        };
        let rect = ScreenRect {
            x: 400.0,
            y: 50.0,
            width: 100.0,
            height: 20.0,
        };

        // Act
        let position = compute_position_near_rect(rect, (320, 200), &[retina]);

        // Assert: 中心 450 点、窗口宽 320 点 → x = 450 - 160 = 290；下沿 70 + 8 → y = 78。
        //         若把选区乘 scale 算成物理像素（旧实现的做法）会得到 (580,148)；tao 对 Logical
        //         是原样透传，物理值当逻辑点下发会落到相邻屏/屏外，故两者必须区分。
        assert_eq!(position, Some(LogicalPosition::new(290, 78)));
    }

    /// 1x 主屏 + 2x 副屏：选区在副屏时交出副屏的逻辑点（= 物理 ÷ 副屏 scale），
    /// 与「窗口此刻停在哪块屏、当前 scale 是几」无关。
    #[test]
    fn position_on_mixed_scale_displays_returns_target_display_logical_points() {
        // Arrange: 副屏物理边界 (3840,0,2560,1440)、scale=2 ⇒ 逻辑 (1920,0,1280,720)
        let secondary_2x = MonitorSpec {
            x: 3840,
            y: 0,
            width: 2560,
            height: 1440,
            scale_factor: 2.0,
        };
        let monitors = [primary_monitor(), secondary_2x];
        let rect = ScreenRect {
            x: 2000.0,
            y: 100.0,
            width: 200.0,
            height: 20.0,
        };

        // Act
        let monitor = find_monitor_for_rect(rect, &monitors);
        let position = compute_position_near_rect(rect, (320, 200), &monitors);

        // Assert: 逻辑 x = (2000 + 100) - 160 = 1940，y = (100 + 20) + 8 = 128
        assert_eq!(monitor, Some(secondary_2x), "选区落在 2x 副屏上");
        assert_eq!(position, Some(LogicalPosition::new(1940, 128)));
        assert_eq!(
            position.map(|p| f64::from(p.x) * secondary_2x.scale_factor),
            Some(3880.0),
            "逻辑点 × 目标屏 scale = 物理落点"
        );
    }

    /// 极端 bounds（高度 1e9）：不 panic，且结果被夹紧在屏内。
    #[test]
    fn position_with_extreme_bounds_is_clamped_not_panicking() {
        // Arrange: 高度 1e10 的「选区」。旧实现把 bottom 在 `as i32` 时饱和到 i32::MAX，
        // 再加 ANCHOR_GAP 就会在 debug 下溢出 panic；取 1e9 时旧实现碰不到这个边界，测试判别力不足。
        let rect = ScreenRect {
            x: 100.0,
            y: 100.0,
            width: 200.0,
            height: 1.0e10,
        };

        // Act
        let position = compute_position_near_rect(rect, (320, 200), &[primary_monitor()]);

        // Assert: x = (100 + 100) - 160 = 40；翻到上方的 y = 100 - 8 - 200 被夹到 0
        assert_eq!(position, Some(LogicalPosition::new(40, 0)));
    }

    /// 极端宽度（1e10）同样不 panic 并夹在屏内，覆盖水平夹紧路径。
    #[test]
    fn position_with_extreme_width_is_clamped_not_panicking() {
        // Arrange: 选区 x=100、宽 1e10，中心远在屏外
        let rect = ScreenRect {
            x: 100.0,
            y: 100.0,
            width: 1.0e10,
            height: 20.0,
        };

        // Act
        let position = compute_position_near_rect(rect, (320, 200), &[primary_monitor()]);

        // Assert: 水平夹到 [0, 1920 - 320] 的右端 1600；垂直在选区下方 y = 120 + 8 = 128
        assert_eq!(position, Some(LogicalPosition::new(1600, 128)));
    }

    /// Windows 出口：目标屏是 2x、屏幕物理原点在 x=2880（主屏 1.5x 的右侧），
    /// 屏内逻辑偏移要用**目标屏**的 scale 换算，而不是窗口当前所在屏的 scale。
    #[test]
    fn placement_on_non_macos_uses_target_monitor_scale() {
        // Arrange: 副屏 2x，物理原点 (2880,0)，逻辑尺寸 1280×720
        let target = MonitorSpec {
            x: 2880,
            y: 0,
            width: 2560,
            height: 1440,
            scale_factor: 2.0,
        };
        let logical = LogicalPosition::new(1440, 100);

        // Act
        let placed = place_for_platform(logical, target);

        // Assert（非 macOS）：屏内逻辑偏移 (1440-1440, 100-0) × 2 = (0,200)，加物理原点 → (2880,200)
        // Assert（macOS）：直接交逻辑点（tao 对 Logical 透传）
        #[cfg(not(target_os = "macos"))]
        assert_eq!(
            placed,
            tauri::Position::Physical(tauri::PhysicalPosition::new(2880, 200))
        );
        #[cfg(target_os = "macos")]
        assert_eq!(
            placed,
            tauri::Position::Logical(tauri::LogicalPosition::new(1440.0, 100.0))
        );
    }

    /// 光标兜底：1x 单屏与改动前逐值一致（旧实现把同样的数值当物理坐标交出，1x 下等价）。
    #[test]
    fn fallback_on_1x_matches_previous_position() {
        // Act
        let position = center_top_logical(primary_monitor(), 720);

        // Assert: x = (1920 - 720) / 2 = 600；y = 1080 * 0.15 = 162
        assert_eq!(position, LogicalPosition::new(600, 162));
    }

    /// 光标兜底：2x 单屏交出逻辑点，与窗口当前 scale 无关。
    #[test]
    fn fallback_on_retina_returns_logical_points() {
        // Arrange: 物理 3840×2160 @2x ⇒ 逻辑 1920×1080
        let retina = MonitorSpec {
            x: 0,
            y: 0,
            width: 3840,
            height: 2160,
            scale_factor: 2.0,
        };

        // Act
        let position = center_top_logical(retina, 720);

        // Assert: 逻辑 (600,162)。旧实现按物理像素算得 (1560,324) 当物理值下发，会被窗口
        //         scale 2 除成逻辑 (780,162)——偏右 180pt（窗口宽/4），本次修正为真正居中。
        assert_eq!(position, LogicalPosition::new(600, 162));
    }

    /// 选区不在任何显示器内：返回 None，由调用方回退光标定位。
    #[test]
    fn position_without_matching_monitor_returns_none() {
        // Arrange: 选区远离所有显示器
        let off_screen = ScreenRect {
            x: 5000.0,
            y: 5000.0,
            width: 100.0,
            height: 20.0,
        };

        // Act
        let monitor = find_monitor_for_rect(off_screen, &[primary_monitor()]);
        let position = compute_position_near_rect(off_screen, (320, 200), &[primary_monitor()]);

        // Assert
        assert_eq!(monitor, None, "没有显示器包含该选区");
        assert_eq!(position, None, "取不到显示器时不得臆造坐标");
    }

    /// 窗口比显示器还大：贴住显示器起点，不产生负坐标。
    #[test]
    fn position_sticks_to_origin_when_window_wider_than_monitor() {
        // Arrange: 300×300 的小屏 + 320 逻辑宽窗口
        let small = MonitorSpec {
            x: 0,
            y: 0,
            width: 300,
            height: 300,
            scale_factor: 1.0,
        };
        let rect = ScreenRect {
            x: 10.0,
            y: 10.0,
            width: 20.0,
            height: 10.0,
        };

        // Act
        let position = compute_position_near_rect(rect, (320, 200), &[small]);

        // Assert: x 夹到 0；y = (10 + 10) + 8 = 28
        assert_eq!(position, Some(LogicalPosition::new(0, 28)));
    }
}
