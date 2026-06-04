//! 集成测试：主窗口红绿灯（交通灯）按钮的定位坐标。
//!
//! 自绘标题栏 `.qq-titlebar` 高 38px，macOS 红绿灯按钮约 14px。
//! 坐标须把按钮在 38px 栏内几何居中，并给左缩进留位、不与标题文字重叠。

use quickquick_lib::traffic_light_logical_position;

/// 坐标须为约定值：x=18 接近左缩进，y=12 使约 14px 按钮在 38px 栏内几何居中（中线对齐标题文字）。
#[test]
fn traffic_light_position_returns_centered_coords() {
    // Act
    let (x, y) = traffic_light_logical_position();

    // Assert
    assert_eq!(x, 18.0);
    assert_eq!(y, 12.0);
}

/// y 取值须让约 14px 按钮整体落在 38px 标题栏内：顶不越过顶边、底不越过底边。
#[test]
fn traffic_light_y_keeps_button_within_titlebar() {
    // Arrange
    let titlebar_height = 38.0;
    let button_height = 14.0;

    // Act
    let (_, y) = traffic_light_logical_position();

    // Assert
    assert!(y >= 0.0, "按钮顶边不应越过标题栏顶部");
    assert!(
        y + button_height <= titlebar_height,
        "按钮底边不应越过标题栏底部"
    );
}
