//! 预热窗口定位模块
//!
//! 策略：热键触发时将窗口移动到「光标所在显示器水平居中、靠上约 15%」的位置。
//! 若无法获取光标位置或匹配显示器，回退到主显示器；若主显示器也不可用，回退到 (0, 0)。
//!
//! 此模块只做坐标计算，不调用 `set_position`，便于逻辑测试。

use tauri::{PhysicalPosition, WebviewWindow};

/// 窗口尺寸（物理像素）——与 tauri.conf.json 中的 400×600 对应。
const WINDOW_WIDTH: i32 = 400;

/// 窗口顶边距离显示器顶边的比例（约 15%）。
const TOP_RATIO: f64 = 0.15;

/// 根据光标位置计算窗口应放置的物理坐标（使用模块默认宽度 400px）。
///
/// 返回值为绝对屏幕坐标（PhysicalPosition），直接传给 `window.set_position()`。
/// 所有可能失败的操作均有回退，不会 panic。
///
/// 保留为 main 窗口定位的向后兼容 wrapper，供 tray / dock 触发逻辑调用。
#[allow(dead_code)]
pub fn compute_window_position(window: &WebviewWindow) -> PhysicalPosition<i32> {
    compute_window_position_for_width(window, WINDOW_WIDTH)
}

/// 根据光标位置和指定宽度计算窗口应放置的物理坐标。
///
/// 与 `compute_window_position` 逻辑相同，但居中计算使用传入的 `width`
/// 而非模块常量，便于 popover 等不同宽度的窗口复用同一定位策略。
pub fn compute_window_position_for_width(
    window: &WebviewWindow,
    width: i32,
) -> PhysicalPosition<i32> {
    // 1. 获取光标位置
    let cursor = window
        .cursor_position()
        .map(|p| (p.x, p.y))
        .unwrap_or((0.0, 0.0));

    // 2. 找到光标所在显示器（或主显示器回退）
    let monitors = window.available_monitors().unwrap_or_else(|e| {
        eprintln!("[QuickQuick] 获取显示器列表失败，回退主显示器: {e}");
        vec![]
    });
    let target_monitor = find_monitor_at(cursor, &monitors)
        .or_else(|| window.primary_monitor().ok().flatten())
        .or_else(|| monitors.into_iter().next());

    // 3. 计算居中靠上位置
    match target_monitor {
        Some(monitor) => {
            let pos = monitor.position();
            let size = monitor.size();
            center_top_position(pos.x, pos.y, size.width, size.height, width)
        }
        None => PhysicalPosition::new(0, 0),
    }
}

/// 在给定显示器列表中查找包含点 `(x, y)` 的显示器。
fn find_monitor_at((cx, cy): (f64, f64), monitors: &[tauri::Monitor]) -> Option<tauri::Monitor> {
    monitors
        .iter()
        .find(|m| {
            let pos = m.position();
            let size = m.size();
            let left = pos.x as f64;
            let top = pos.y as f64;
            let right = left + size.width as f64;
            let bottom = top + size.height as f64;
            cx >= left && cx < right && cy >= top && cy < bottom
        })
        .cloned()
}

/// 在给定显示器区域内计算水平居中、靠上 TOP_RATIO 的物理坐标。
///
/// 参数均为物理像素值：
/// - `mon_x` / `mon_y`：显示器左上角坐标
/// - `mon_w` / `mon_h`：显示器宽高
/// - `width`：窗口宽度（用于水平居中计算）
fn center_top_position(
    mon_x: i32,
    mon_y: i32,
    mon_w: u32,
    mon_h: u32,
    width: i32,
) -> PhysicalPosition<i32> {
    let x = mon_x + (mon_w as i32 - width) / 2;
    let y = mon_y + (mon_h as f64 * TOP_RATIO) as i32;
    PhysicalPosition::new(x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证水平居中计算：显示器 1920×1080，窗口 400px 宽，x 应为 760
    #[test]
    fn center_top_x_is_centered() {
        // Arrange: 显示器起点 (0,0)，大小 1920×1080，使用默认宽度 400
        // Act
        let pos = center_top_position(0, 0, 1920, 1080, WINDOW_WIDTH);

        // Assert: x = (1920 - 400) / 2 = 760
        assert_eq!(pos.x, 760, "水平居中 x 应为 760");
    }

    /// 验证靠上 15%：1080 * 0.15 = 162
    #[test]
    fn center_top_y_is_fifteen_percent() {
        // Arrange
        // Act
        let pos = center_top_position(0, 0, 1920, 1080, WINDOW_WIDTH);

        // Assert: y = floor(1080 * 0.15) = 162
        assert_eq!(pos.y, 162, "靠上 15% y 应为 162");
    }

    /// 验证多显示器偏移：显示器起点 (1920, 200)，大小 2560×1440
    #[test]
    fn center_top_accounts_for_monitor_offset() {
        // Arrange
        // Act
        let pos = center_top_position(1920, 200, 2560, 1440, WINDOW_WIDTH);

        // Assert: x = 1920 + (2560 - 400) / 2 = 1920 + 1080 = 3000
        //         y = 200 + floor(1440 * 0.15) = 200 + 216 = 416
        assert_eq!(pos.x, 3000, "多显示器偏移 x 应为 3000");
        assert_eq!(pos.y, 416, "多显示器偏移 y 应为 416");
    }

    /// 验证 find_monitor_at：空列表返回 None
    #[test]
    fn find_monitor_at_no_monitors_returns_none() {
        // Arrange
        let monitors: Vec<tauri::Monitor> = vec![];

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
}
