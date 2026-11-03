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
//! **判屏（哪块屏上有光标）是另一处平台差异，同样按平台分开。** tao 的 macOS
//! `cursor_position()` 返回「左上原点点 × 主屏 scale」，而 `Monitor` 的物理边界是「点 × 本屏
//! scale」：混合 DPI（如 2x 主屏 + 1x 副屏）下两者不在同一坐标空间，光标点被主屏 scale 放大后
//! 会超出 1x 副屏的物理边界，判屏必然落空并回退主屏。因此 macOS 由 [`cursor_monitor`] 走 AppKit
//! 路径（`NSScreen` + `NSEvent::mouseLocation`，全程点空间），不经过 tao 的光标与显示器列表；
//! Windows 的 `cursor_position()` 与 `Monitor` 边界同为物理像素，仍在物理空间用 `find_monitor_at`
//! 判定。
//!
//! 输入侧因此还要分清两套单位：AX 的选区矩形（[`ScreenRect`]）本身就是逻辑点；tauri 的
//! `Monitor` 位置与尺寸是物理像素，用 [`MonitorSpec::logical_bounds`] 除回逻辑点后参与计算
//! （AppKit 给的点坐标也按同一关系包装成 [`MonitorSpec`]，见 [`monitor_spec_from_points`]）。

use tauri::{LogicalPosition, Position, WebviewWindow};

use crate::selection::ScreenRect;

/// 窗口顶边距离显示器顶边的比例（约 15%）。
const TOP_RATIO: f64 = 0.15;

/// 回退路径的完整放置：光标所在屏水平居中、靠上约 15%。
///
/// 返回**逻辑点坐标**与它所在的目标屏（交给 [`place_for_platform`] 做平台出口）。
/// 屏内计算一律用目标屏的逻辑尺寸：物理尺寸先除回点、窗口宽度本就是逻辑值，落点与
/// 「窗口此刻停在哪块屏、当前 scale 是几」无关。
pub fn placement_for_width(
    window: &WebviewWindow,
    width: i32,
) -> (LogicalPosition<i32>, Option<MonitorSpec>) {
    // 1. 判屏：坐标系按平台分流，见 [`cursor_monitor`]。
    // 2. 取不到光标所在屏时回退：主显示器 → 显示器列表首个 → (0, 0) 原点。
    let target_monitor = cursor_monitor(window)
        .or_else(|| {
            window
                .primary_monitor()
                .ok()
                .flatten()
                .map(|monitor| monitor_spec(&monitor))
        })
        .or_else(|| monitor_specs(window).first().copied());

    // 3. 计算居中靠上位置（逻辑点）
    match target_monitor {
        Some(monitor) => (center_top_logical(monitor, width), Some(monitor)),
        None => (LogicalPosition::new(0, 0), None),
    }
}

/// 光标所在显示器；取不到光标或显示器列表时返回 `None`，由调用方回退主显示器。
///
/// 坐标空间必须按平台分开：macOS 的 AppKit 几何与鼠标位置都是点（见模块头），
/// 其它平台的 `cursor_position()` 与 `Monitor` 边界同为物理像素。
#[cfg(target_os = "macos")]
pub fn cursor_monitor(_window: &WebviewWindow) -> Option<MonitorSpec> {
    use objc2::MainThreadMarker;
    use objc2_app_kit::{NSEvent, NSScreen};

    // 判屏用不到窗口句柄：macOS 侧的显示器几何与鼠标位置都直接从 AppKit 读，
    // 不经过 tao 的 cursor_position / available_monitors。
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("[QuickQuick] 判屏：当前不在主线程，无法读取 NSScreen / NSEvent");
        return None;
    };

    let screens: Vec<AppKitScreen> = NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let frame = screen.frame();
            AppKitScreen {
                origin_x: frame.origin.x,
                origin_y: frame.origin.y,
                width: frame.size.width,
                height: frame.size.height,
                scale: screen.backingScaleFactor(),
            }
        })
        .collect();
    if screens.is_empty() {
        eprintln!("[QuickQuick] 判屏：NSScreen 列表为空");
        return None;
    }

    let mouse = NSEvent::mouseLocation();
    let (cursor, monitors) = appkit_layout_to_points(&screens, (mouse.x, mouse.y));
    find_monitor_containing_point_pts(cursor, &monitors)
}

/// AppKit 报告的一块屏幕：frame 以主屏左下角为原点、y 向上，单位为点。
///
/// 独立成纯数据类型，让翻转逻辑能在没有 `NSScreen` 的环境里被单测覆盖（见
/// [`appkit_layout_to_points`]）。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppKitScreen {
    /// frame 左下角 x（点）。
    pub origin_x: f64,
    /// frame 左下角 y（点，主屏为 0，上方屏为正，下方屏为负）。
    pub origin_y: f64,
    /// frame 宽（点）。
    pub width: f64,
    /// frame 高（点）。
    pub height: f64,
    /// `backingScaleFactor`。
    pub scale: f64,
}

/// 把 AppKit 的屏幕布局与鼠标位置换算到**左上原点的点坐标**空间。
///
/// 纵向翻转基准取 `screens[0]` 的高度：AppKit 以主屏左下角为原点，主屏即带菜单栏的那块，
/// 它的顶边在左上原点空间里恰好是 y = 0。每屏的顶边 `top = mainH - (origin_y + height)`，
/// 鼠标的 `y = mainH - mouse_y`。返回的 `MonitorSpec` 列表与鼠标点处在同一空间，
/// 可直接交给 [`find_monitor_containing_point_pts`]。
///
/// `screens` 为空时调用方应先行拦截；这里对空切片直接返回空列表，避免越界。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[must_use]
pub fn appkit_layout_to_points(
    screens: &[AppKitScreen],
    mouse_appkit: (f64, f64),
) -> ((f64, f64), Vec<MonitorSpec>) {
    let Some(main) = screens.first() else {
        return ((mouse_appkit.0, mouse_appkit.1), Vec::new());
    };
    let main_height = main.height;
    let cursor = (mouse_appkit.0, main_height - mouse_appkit.1);
    let monitors = screens
        .iter()
        .map(|screen| {
            let top = main_height - (screen.origin_y + screen.height);
            monitor_spec_from_points(
                screen.origin_x,
                top,
                screen.width,
                screen.height,
                screen.scale,
            )
        })
        .collect();
    (cursor, monitors)
}

/// 光标所在显示器；取不到光标或显示器列表时返回 `None`，由调用方回退主显示器。
///
/// 物理空间判定：这里的 `cursor_position()`（GetCursorPos）与 `Monitor` 边界（rcMonitor）
/// 是同一套物理像素坐标，直接比较即正确。
#[cfg(not(target_os = "macos"))]
pub fn cursor_monitor(window: &WebviewWindow) -> Option<MonitorSpec> {
    let Some(cursor) = window.cursor_position().ok() else {
        eprintln!("[QuickQuick] 判屏：读取光标位置失败，回退主显示器");
        return None;
    };
    find_monitor_at((cursor.x, cursor.y), &monitor_specs(window))
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

/// 把「左上原点点坐标」的显示器几何包装成 [`MonitorSpec`]。
///
/// `MonitorSpec` 的字段语义是物理像素，故这里把点乘上该屏 scale 存进去，
/// [`MonitorSpec::logical_bounds`] 再除回同一组点坐标。AppKit 给出的屏幕矩形本来就是点，
/// 需要这层包装才能与其它来源的 [`MonitorSpec`] 走同一套逻辑点计算。
///
/// scale 非法（非有限或非正）时两侧都退成 1.0（见 [`normalized_scale`]），点坐标原样保留。
///
/// 生产路径上只有 macOS 的 [`cursor_monitor`] 用它（其它平台的判屏留在物理空间），
/// 所以在那些平台它是测试专用的纯函数。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[must_use]
pub fn monitor_spec_from_points(
    left_pt: f64,
    top_pt: f64,
    width_pt: f64,
    height_pt: f64,
    scale_factor: f64,
) -> MonitorSpec {
    let scale = normalized_scale(scale_factor);
    // 点 → 物理像素：乘本屏 scale 后四舍五入；`as i32` 在越界或 NaN 时饱和，不 panic。
    let to_physical = |points: f64| (points * scale).round() as i32;
    MonitorSpec {
        x: to_physical(left_pt),
        y: to_physical(top_pt),
        width: to_physical(width_pt),
        height: to_physical(height_pt),
        scale_factor,
    }
}

/// 参与换算的缩放比；非有限或非正的值退成 1.0，避免除零与 NaN 扩散到坐标里。
fn normalized_scale(scale_factor: f64) -> f64 {
    if scale_factor.is_finite() && scale_factor > 0.0 {
        scale_factor
    } else {
        1.0
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
/// 只在 Windows 等平台使用：那里的 `cursor_position()` 与 `Monitor` 边界同为物理像素，
/// 物理空间比较才成立。macOS 走 [`cursor_monitor`] 的 AppKit 点空间路径。
#[cfg(not(target_os = "macos"))]
fn find_monitor_at((cx, cy): (f64, f64), monitors: &[MonitorSpec]) -> Option<MonitorSpec> {
    monitors.iter().copied().find(|monitor| {
        let left = f64::from(monitor.x);
        let top = f64::from(monitor.y);
        let right = left + f64::from(monitor.width);
        let bottom = top + f64::from(monitor.height);
        cx >= left && cx < right && cy >= top && cy < bottom
    })
}

/// 在给定显示器列表中查找**点空间**包含点 `(px, py)` 的显示器。
///
/// 边界约定与 `find_monitor_at` 一致：半开区间 `[left, right) × [top, bottom)`。
/// 区别只在坐标空间——这里先把各屏物理边界经 [`MonitorSpec::logical_bounds`] 还原成点，
/// 混合 DPI 下每块屏的边界因此都与光标点处在同一空间：若拿点去比物理边界，2x 主屏会把
/// (5000, 600) 这种点放大成 10000，落在 1x 副屏物理范围 [4616, 7176) 之外而误判为「不在任何屏」。
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
#[must_use]
pub fn find_monitor_containing_point_pts(
    (px, py): (f64, f64),
    monitors: &[MonitorSpec],
) -> Option<MonitorSpec> {
    monitors.iter().copied().find(|monitor| {
        let (left, top, width, height) = monitor.logical_bounds();
        px >= left && px < left + width && py >= top && py < top + height
    })
}

/// 在给定显示器区域内计算水平居中、靠上 TOP_RATIO 的**逻辑点**坐标。
///
/// 参数均为逻辑点：`mon_x` / `mon_y` 是显示器左上角，`mon_w` / `mon_h` 是显示器宽高，
/// `width` 是窗口逻辑宽度。
///
/// 内部退到 f64 再截断：避免整数除法丢掉奇数宽度的半个像素，并且在极端显示器尺寸下
/// 不会整数溢出。`as i32` 在越界时饱和，不 panic。
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
/// 算出来的结果与「窗口当前在哪块屏、当前 scale 是几」无关。
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
        normalized_scale(self.scale_factor)
    }

    /// 显示器边界的逻辑点表示。
    ///
    /// 为什么可以直接除：tauri 的显示器位置与尺寸由 `CGDisplayBounds`（点）× scale_factor
    /// 得到，因此「物理 = 点 × 本屏 scale」在全局坐标空间里逐屏成立；AppKit 来源的点几何
    /// 由 [`monitor_spec_from_points`] 按同一关系包装，所以这里对两种来源都成立。
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
    ///
    /// 物理空间判屏只在 Windows 等非 macOS 平台使用，故单测随函数一起按平台门禁。
    #[cfg(not(target_os = "macos"))]
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
        //         若把选区乘 scale 算成物理像素会得到 (580,148)；tao 对 Logical 是原样透传，
        //         物理值当逻辑点下发会落到相邻屏或屏外，故两者必须区分。
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
        // Arrange: 高度 1e10 的选区。bottom 超出 i32 时 `as i32` 饱和到 i32::MAX，若在 i32 上
        // 再加 ANCHOR_GAP 就会在 debug 下溢出 panic；取 1e10 能越过这个边界，因此测试有判别力。
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

    /// 1x 单屏：逻辑点直接输出，居中结果逐值可算。
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

        // Assert: 逻辑点 (600,162)。屏内计算一律用逻辑尺寸，窗口 scale 不参与换算，因此 2x 屏
        //         与 1x 屏上的居中结果一致。
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

    // ===== 点空间几何：点坐标 ↔ MonitorSpec / 点空间判屏 =====

    /// 1x 屏：物理字段与点坐标相同，`logical_bounds` 原样还原。
    #[test]
    fn monitor_spec_from_points_round_trips_points_on_1x() {
        // Arrange & Act
        let spec = monitor_spec_from_points(4616.0, 0.0, 2560.0, 1440.0, 1.0);

        // Assert
        assert_eq!(
            (spec.x, spec.y, spec.width, spec.height),
            (4616, 0, 2560, 1440),
            "1x 下物理字段等于点坐标"
        );
        assert_eq!(spec.logical_bounds(), (4616.0, 0.0, 2560.0, 1440.0));
    }

    /// 2x 屏：物理字段存「点 × 2」，`logical_bounds` 除回原始点坐标。
    #[test]
    fn monitor_spec_from_points_round_trips_points_on_2x() {
        // Arrange & Act
        let spec = monitor_spec_from_points(0.0, 0.0, 1028.0, 664.5, 2.0);

        // Assert
        assert_eq!(
            (spec.x, spec.y, spec.width, spec.height),
            (0, 0, 2056, 1329),
            "2x 下物理字段是点 × 2"
        );
        assert_eq!(spec.logical_bounds(), (0.0, 0.0, 1028.0, 664.5));
    }

    /// 非法 scale（0）退成 1.0：点坐标原样保留，不产生除零或 NaN。
    #[test]
    fn monitor_spec_from_points_falls_back_to_unit_scale_when_invalid() {
        // Arrange & Act
        let spec = monitor_spec_from_points(100.0, 50.0, 800.0, 600.0, 0.0);

        // Assert
        assert_eq!(
            (spec.x, spec.y, spec.width, spec.height),
            (100, 50, 800, 600)
        );
        assert_eq!(spec.logical_bounds(), (100.0, 50.0, 800.0, 600.0));
    }

    /// 2x 主屏：逻辑 2056×1329（物理 4112×2658），左上原点 (0,0)。
    fn mixed_primary_2x() -> MonitorSpec {
        monitor_spec_from_points(0.0, 0.0, 2056.0, 1329.0, 2.0)
    }

    /// 2x 非原点副屏（逻辑原点 x=2056，2560×1440）。
    fn mixed_secondary_2x() -> MonitorSpec {
        monitor_spec_from_points(2056.0, 0.0, 2560.0, 1440.0, 2.0)
    }

    /// 1x 副屏（逻辑原点 x=4616，2560×1440）。
    fn mixed_tertiary_1x() -> MonitorSpec {
        monitor_spec_from_points(4616.0, 0.0, 2560.0, 1440.0, 1.0)
    }

    /// 混合 DPI：点落在 1x 副屏的逻辑范围内时，命中该屏。
    #[test]
    fn find_monitor_containing_point_pts_finds_1x_screen_in_mixed_dpi() {
        // Arrange: 2x 主屏 + 2x 副屏 + 1x 副屏；光标点 (5896, 720) 在 1x 屏逻辑范围内
        let monitors = [
            mixed_primary_2x(),
            mixed_secondary_2x(),
            mixed_tertiary_1x(),
        ];

        // Act
        let monitor = find_monitor_containing_point_pts((5896.0, 720.0), &monitors);

        // Assert
        assert_eq!(monitor, Some(mixed_tertiary_1x()), "点空间判屏应命中 1x 屏");
    }

    /// 非原点的 2x 屏：点空间判屏直接命中。
    #[test]
    fn find_monitor_containing_point_pts_finds_offset_2x_screen() {
        // Arrange
        let monitors = [
            mixed_primary_2x(),
            mixed_secondary_2x(),
            mixed_tertiary_1x(),
        ];

        // Act
        let monitor = find_monitor_containing_point_pts((3336.0, 720.0), &monitors);

        // Assert
        assert_eq!(
            monitor,
            Some(mixed_secondary_2x()),
            "点空间判屏应命中非原点 2x 屏"
        );
    }

    /// 所有屏之外的点返回 None，由调用方回退主屏。
    #[test]
    fn find_monitor_containing_point_pts_returns_none_outside_all_screens() {
        // Arrange: 最右屏（1x，右边缘 7176）之外
        let monitors = [
            mixed_primary_2x(),
            mixed_secondary_2x(),
            mixed_tertiary_1x(),
        ];

        // Act
        let monitor = find_monitor_containing_point_pts((7200.0, 720.0), &monitors);

        // Assert
        assert_eq!(monitor, None, "屏外的点不得臆造屏幕");
    }

    /// AppKit 布局：主屏 2x，副屏 1x 且 frame.origin.y 为负（位于主屏下方）。
    fn appkit_mixed_layout() -> [AppKitScreen; 2] {
        [
            AppKitScreen {
                origin_x: 0.0,
                origin_y: 0.0,
                width: 2056.0,
                height: 1329.0,
                scale: 2.0,
            },
            AppKitScreen {
                origin_x: 4616.0,
                origin_y: -111.0,
                width: 2560.0,
                height: 1440.0,
                scale: 1.0,
            },
        ]
    }

    /// AppKit 翻转：副屏顶边在左上原点空间为 0（1329 - (-111 + 1440) = 0）。
    #[test]
    fn appkit_layout_places_lower_screen_top_at_zero() {
        // Arrange
        let screens = appkit_mixed_layout();

        // Act
        let (_, monitors) = appkit_layout_to_points(&screens, (5896.0, 609.0));

        // Assert
        assert_eq!(monitors[1].logical_bounds(), (4616.0, 0.0, 2560.0, 1440.0));
    }

    /// AppKit 翻转：鼠标左下原点 y=609 → 左上原点 y=720，并落在 1x 副屏内。
    #[test]
    fn appkit_layout_flips_mouse_and_resolves_lower_screen() {
        // Arrange
        let screens = appkit_mixed_layout();

        // Act
        let (cursor, monitors) = appkit_layout_to_points(&screens, (5896.0, 609.0));
        let monitor = find_monitor_containing_point_pts(cursor, &monitors);

        // Assert
        assert_eq!(cursor, (5896.0, 720.0));
        assert_eq!(monitor, Some(monitors[1]), "翻转后的光标应落在 1x 副屏");
    }

    /// 空布局：不 panic，返回空列表。
    #[test]
    fn appkit_layout_with_no_screens_returns_empty() {
        // Arrange & Act
        let (_, monitors) = appkit_layout_to_points(&[], (10.0, 10.0));

        // Assert
        assert!(monitors.is_empty());
    }

    /// 边界半开：右边缘与下边缘上的点不属于该屏。
    #[test]
    fn find_monitor_containing_point_pts_treats_right_and_bottom_edges_as_exclusive() {
        // Arrange
        let monitors = [primary_monitor()];

        // Act
        let inside = find_monitor_containing_point_pts((1919.0, 1079.0), &monitors);
        let at_right = find_monitor_containing_point_pts((1920.0, 500.0), &monitors);
        let at_bottom = find_monitor_containing_point_pts((500.0, 1080.0), &monitors);

        // Assert: [left, right) × [top, bottom)
        assert_eq!(inside, Some(primary_monitor()));
        assert_eq!(at_right, None, "右边界不包含");
        assert_eq!(at_bottom, None, "下边界不包含");
    }

    /// 屏外点不属于任何显示器。
    #[test]
    fn find_monitor_containing_point_pts_returns_none_outside_all_monitors() {
        // Arrange
        let monitors = [primary_monitor()];

        // Act
        let monitor = find_monitor_containing_point_pts((-1.0, 500.0), &monitors);

        // Assert
        assert_eq!(monitor, None, "屏外点应返回 None");
    }

    /// 空显示器列表返回 None。
    #[test]
    fn find_monitor_containing_point_pts_with_empty_list_returns_none() {
        // Arrange
        let monitors: Vec<MonitorSpec> = vec![];

        // Act
        let monitor = find_monitor_containing_point_pts((100.0, 100.0), &monitors);

        // Assert
        assert_eq!(monitor, None, "空列表应返回 None");
    }
}
