---
id: TV1-F1-S01-code
type: coding_record
level: 小功能
parent: TV1-F1
status: 通过
commit: PENDING
acceptance_ids: [TV1-F1-A01, TV1-F1-A02, TV1-F1-A03]
---

# TV1-F1-S01 编码留痕：Lingva 默认源替代 MyMemory

## 做了什么

用免 key 的 **Lingva**（pot-app 托管实例，实测准）替换默认翻译源 MyMemory（实测「冰川→Bing Chuan」垃圾）。严格 TDD（红-绿-重构）落地：

1. 新增 `LingvaProvider`（无凭据，needs_key=false），实现薄 provider 三职责（capability / build_request / parse_response）。
2. 移除 `MyMemoryProvider`（struct+impl+其单测）、registry/build_provider/credential_schema 的 mymemory 项及关联的 `NeedEmail` 提示分支。
3. 默认源切 lingva；持久化的 `selected_provider` 若不在注册表 id 集合内（含旧值 mymemory、任意未知 id）→ 解析回退 lingva 并在读路径检测到回退时持久化修正。
4. 前端 `translate-page.test.tsx` 的 provider mock 与默认源断言由 mymemory 改为 lingva。

## 关键决策与理由

- **独立实现、不抄 pot 源代码（许可红线）**：Lingva 端点 `GET https://lingva.pot-app.com/api/v1/{src}/{tgt}/{text}` 与取译文字段 `translation` 属公开 HTTP 互操作协议事实，按实测 ground truth 用原创 Rust 编写；代码注释标注 **Lingva 协议来源**（开源 Google 翻译前端的无认证 GET 接口），不写 pot 源码 URL、未打开 pot 代码。
- **迁移判定以「是否在注册表」为准，而非硬编码判 mymemory**：抽出纯函数 `resolve_provider_or_fallback(stored) -> String`（在 `ipc/translate.rs`），`stored` 在 `registry()` id 集合内则原样返回，否则回退 `DEFAULT_PROVIDER_ID="lingva"`。将来删任何源都安全，且可单测。
- **读路径只在发生回退时才写回**：`get_selected_provider_impl` 仅当解析值与存储值不同（即存储值非法）时才 `save`，避免每次读取触发磁盘写。
- **语言码映射按实测协议**：lingva 对 auto/zh/en 直传（实测 `/zh/en/冰川`→glacier、`/en/zh/hello%20world`→你好世界），zh-TW→`zh_HANT`，其余沿用直传码；单测用 mock 不打真网。
- **移除 MyMemory 的连带清理**：`NeedEmail` 提示类型仅服务于 mymemory 配额提升，随源移除而删除；`on_quota_or_failure` 简化为配额/鉴权失败统一返回 `NeedKey`，不留指向已删除源的死分支；`percent_encode_langpair`（仅 MyMemory 的 langpair 用）一并删除。
- **未触碰 baidu / deepl_free / google** 的实现逻辑（仅它们所在文件的 mymemory 旁支被清理）。

## 改动文件清单

| 文件 | 改动 | 为什么 |
|---|---|---|
| `src-tauri/src/translate/providers.rs` | 新增 `LingvaProvider`；删 `MyMemoryProvider`+`map_mymemory_error`+`percent_encode_langpair`；registry/build_provider 用 lingva；`UserPromptKind` 删 `NeedEmail`、`on_quota_or_failure` 简化；新增 3 个 lingva 单测 | A01/A02 核心实现 + 移除 MyMemory |
| `src-tauri/src/translate/lang.rs` | `map_for_mymemory`→`map_for_lingva`；map_lang_for_provider 分支改 lingva；更新两个映射单测 | lingva 语言码映射（实测协议） |
| `src-tauri/src/translate/credential.rs` | 删 credential_schema 的 mymemory(email) 分支（lingva 免 key 落空 schema） | 免 key 源无凭据字段 |
| `src-tauri/src/ipc/translate.rs` | 新增 `DEFAULT_PROVIDER_ID` 常量 + 纯函数 `resolve_provider_or_fallback`；新增 A03 迁移单测；测试 helper/响应改 lingva 格式 | A03 设置迁移（可单测纯函数） |
| `src-tauri/src/ipc/settings.rs` | `get_selected_provider_impl` 接入迁移回退 + 写路径持久化修正 | A03 读路径迁移 |
| `src-tauri/src/settings.rs` | `default_selected_provider` mymemory→lingva；更新两个默认值断言 | 默认源切 lingva |
| `src-tauri/tests/providers.rs` | MyMemory 集成测试块→Lingva（capability/build_request/parse 成功+缺字段+非法 JSON）；NeedEmail 测试→keyless 源 NeedKey | 同步移除 MyMemory |
| `src-tauri/tests/translate.rs` | registry/needs_key/lang_norm/credential_schema 的 mymemory 断言改 lingva；retry 测试标签改 lingva | 同步移除 MyMemory |
| `src-tauri/tests/ipc_translate.rs` | write_settings 改 lingva；FakeExecutor 响应改 `{"translation":...}` | 迁移后走 lingva provider |
| `src-tauri/tests/ipc_settings.rs` | 合法 id roundtrip 改 lingva；providers 列表断言改 lingva 且不含 mymemory；新增旧值 mymemory→lingva 迁移集成测试 | A03 迁移集成覆盖 |
| `src/panels/translate/translate-page.test.tsx` | provider mock + 默认源断言 mymemory→lingva；注释更新 | 前端默认源对齐 |

## 自测结论

- **红→绿**：先写 `providers_registry_has_lingva_no_mymemory` / `lingva_build_request_url_and_parse_translation` / `selected_provider_migrates_unknown_to_lingva` 三个测试，RED 阶段因 `LingvaProvider` 未实现编译失败（功能缺口，非环境错）；实现后转绿。
- **cargo test**：exit=0，全部 32 个测试套件 `test result: ok`，含三个验收测试名 `... ok`（证据见 `artifacts/cargo-test.log`）。
- **clippy**：`cargo clippy --all-targets -- -D warnings` exit=0，无警告（`artifacts/cargo-clippy.log`）。
- **前端**：`pnpm test` Test Files 51 passed (51) / Tests 462 passed (462)（`artifacts/pnpm-test.log`）；`tsc --noEmit` No errors found。
- **安全（TV1-A-SEC）**：lingva needs_key=false，build_provider 不读凭据；providers.rs 无 eprintln（grep 无命中），错误处理不打印待译 text/译文。
- **自检**：无 TODO/FIXME、无装饰性分隔注释（含测试文件），过滤测试名真命中。
