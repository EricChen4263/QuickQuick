---
id: TV4-F2-S01
type: coding
parent: TV4-F2
commit: 7b34b76
acceptance_ids: [TV4-F2-A01]
---

# TV4-F2-S01 编码留痕：ECDICT + 有道词典模式（JSON 词条源 → Dict）

## 目标

新增两个词典源，返回结构化 `TranslateResponse::Dict(DictEntry)`：

- **ecdict**（免 key，pot 自建）：`POST https://pot-app.com/api/dict`，body `{"word":"..."}`，解析 ECDICT 英汉词条行。
- **youdao_dict**（需 key，复用有道签名）：同有道翻译端点/签名；`isWord===true` 且含 `basic` 时解析为 Dict，否则回退 Plain。

registry 19 → 21。

## 改动文件

| 文件 | 改动 |
|---|---|
| `src-tauri/src/translate/providers.rs` | 新增 `EcdictProvider`、`YoudaoDictProvider` + 解析辅助纯函数；`use` 引入 `DictEntry`/`PosDefinition`；`build_provider` 加 `ecdict`/`youdao_dict` 分支；`registry()` 加两源；新增 9 个单测（含 3 冻结 + 错误分支 + 非词回退 + sentinel 安全） |
| `src-tauri/src/translate/credential.rs` | `youdao_dict` 复用有道 schema（`app_key`/`app_secret`，与 `youdao` 合并同一分支）；ecdict 免 key 落 `_ => vec![]` |
| `src-tauri/tests/translate.rs` | 注册表数量断言 `static_registry_lists_nineteen_providers`(19) → `static_registry_lists_twenty_one_providers`(21)，并更新文档注释列出新源 |

## 关键实现决策

### ECDICT 字段映射（ECDICT 行结构 → DictEntry）

- `phonetic` → `DictEntry.phonetic`（空串过滤为 None）。
- `translation`（英汉释义，多词性按 `\n` 分隔）→ 按行拆分，每行按词性前缀分组为 `PosDefinition`。
- `exchange`（如 `s:glaciers/p:glacial`）→ `parse_ecdict_exchange` 取各项 `:` 后的值为词形列表 `inflections`。
- 无 `translation`（未收录/非词）→ `TranslateError::ParseError`，不 panic。
- `is_unofficial=true`（pot 自建公共服务，同 lingva 处理，与 lingva 同源 pot-app 域名）。

### 有道词典模式（isWord 分流）

- 端点/签名**完全复用** `youdao_sign`（SHA256 signType=v3）+ `map_youdao_error`，不另起算法。`build_request` 与 `YoudaoProvider` 结构一致，仅 capability id 为 `youdao_dict`。
- parse 分流：
  - `errorCode != "0"` → 复用 `map_youdao_error` 归一。
  - `isWord===true` 且 `basic` 为 object → `parse_youdao_basic` → `Dict`。
  - 否则（非词/无 basic）→ 取 `translation[*]` 拼接 → `Plain`。
- `parse_youdao_basic` 映射：音标优先级 `us-phonetic → phonetic → uk-phonetic`；`explains` 按词性分组；`wfs[*].wf` 取 `name: value`（如 `复数: glaciers`）为词形。

### 词性分组（DRY 共用）

`group_definitions_by_pos` + `split_pos_prefix` + `is_pos_token` 三个纯函数被 ECDICT 与有道词典共用：识别首 token 形如 `n.`/`vt.`/`adj.`（ASCII 字母 + `.` 结尾）为词性前缀，同词性合并到同一 `PosDefinition`。

## 安全

- 有道词典 `app_secret` 经 schema `is_secret=true` 加密存储；`build_request`/签名代码无任何 `eprintln/println/log/dbg`。
- sentinel 测试 `youdao_dict_build_request_does_not_leak_secret`：用 `SENTINEL_DEADBEEF` 脏值断言请求 body `!contains`，并独立复算 `youdao_sign` 锚定签名（非循环论证）。

## 冻结测试转绿证据

`artifacts/frozen-tests-green.log`（rtk proxy 原始输出）：

```
test translate::providers::tests::ecdict_build_and_parse_dict ... ok
test translate::providers::tests::youdao_dict_falls_back_to_plain_when_not_word ... ok
test translate::providers::tests::youdao_dict_parses_basic_to_dict ... ok
test result: ok. 91 passed; 0 failed; ...
```

## 坑 / 注意

- registry 数量断言在 `tests/translate.rs`（集成测试），函数名一并改名以反映新数量（避免名实不符的死命名）。
- ecdict 免 key 复用 `_ => vec![]` 的 schema 兜底，无需新增 credential 分支。

## 全量验证

- **全量 `cargo test`**：exit 0，全部 `test result: ok`（0 failed），lib 块 256 passed（含本小功能新增 9 个测试）。连跑 3 次均全绿、无 flaky。原始证据 `artifacts/full-test.log`、`artifacts/frozen-tests-green.log`。
- **`cargo test --release`**：exit 0，无 FAILED。证据 `artifacts/release-test.log`。
- **`cargo clippy --all-targets -- -D warnings`**：exit 0，无警告。证据 `artifacts/clippy.log`。
- **修复点**：新增免 key 源 `ecdict` 后，集成测试 `static_registry_keyed_providers_need_key` 的 `keyless_ids` 集合补入 `ecdict`（否则被误判为需 key，hints「新增免 key 源须同批更新该集合」）。

## 自检（code-standards 逐项）

- 函数 ≤50 行、嵌套 ≤3 层：解析拆为 `parse_youdao_basic`/`group_definitions_by_pos`/`split_pos_prefix`/`is_pos_token`/`parse_ecdict_exchange` 小纯函数。
- DRY：词性分组三函数 ECDICT 与有道词典共用；有道词典复用 `youdao_sign`/`map_youdao_error`/`current_unix_secs`，不另起算法。
- 安全：`app_secret` is_secret 加密存储；provider 代码无 `eprintln/println/log/dbg`；sentinel 脏值证否泄露。
- 无装饰性分隔注释、无 TODO/FIXME（grep 已核）。
- 注释写「为什么」（字段映射来源、isWord 分流理由）。
</content>
</invoke>
