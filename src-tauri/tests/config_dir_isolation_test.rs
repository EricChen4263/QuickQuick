//! 集成测试：dev/release 配置目录隔离的纯路径逻辑
//!
//! 覆盖 f6-s13：debug 构建在 app_config_dir 下追加 `dev` 子目录，
//! release 构建不追加，从而隔离两套密钥体系下的 SQLCipher 数据库。
//!
//! 测试约定：文件名含 `config_dir_isolation` 确保 verify 命中。
//! 目录解析整体依赖 Tauri 运行时（不可单测），此处仅测抽出的纯路径逻辑。

use quickquick_lib::ipc::settings::apply_dev_subdir;
use std::path::Path;

#[test]
fn apply_dev_subdir_appends_dev_in_debug_build() {
    let base = Path::new("/cfg/com.quickquick.app");
    let resolved = apply_dev_subdir(base, true);
    assert_eq!(resolved, Path::new("/cfg/com.quickquick.app/dev"));
}

#[test]
fn apply_dev_subdir_keeps_base_in_release_build() {
    let base = Path::new("/cfg/com.quickquick.app");
    let resolved = apply_dev_subdir(base, false);
    assert_eq!(resolved, Path::new("/cfg/com.quickquick.app"));
}
