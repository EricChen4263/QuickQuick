// Tauri 构建脚本：生成必要的平台资源与元数据
fn main() {
    // tauri.conf.json 的 bundle.resources 声明了 resources/ecdict.db；
    // 真库由 CI 在打包前用 tools/gen_ecdict_db.py 生成（不入 git，见 .gitignore）。
    // 本地开发/测试无真库时，tauri_build 会因资源路径不存在而失败——
    // 故确保占位文件存在使编译通过；运行时 EcdictDb::lookup 对空库返回错误而非 panic。
    ensure_ecdict_db_placeholder();
    println!("cargo:rerun-if-changed=resources/ecdict.db");

    tauri_build::build()
}

/// 确保 `resources/ecdict.db` 存在：缺失时创建空占位文件（满足 tauri 资源路径校验）。
///
/// 只在文件不存在时创建空文件，绝不覆盖 CI 生成的真库。创建失败仅打印警告、不中断构建。
fn ensure_ecdict_db_placeholder() {
    let path = std::path::Path::new("resources/ecdict.db");
    if path.exists() {
        return;
    }
    if let Some(parent) = path.parent() {
        if let Err(e) = std::fs::create_dir_all(parent) {
            println!("cargo:warning=无法创建 resources 目录: {e}");
            return;
        }
    }
    if let Err(e) = std::fs::write(path, []) {
        println!("cargo:warning=无法创建 ecdict.db 占位文件: {e}");
    }
}
