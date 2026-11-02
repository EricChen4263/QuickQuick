---
status: done
category: implementation
subject: 翻译交互 P2 选中图标（默认关闭）与 P4 浮窗词典视图
summary: P2：设置开关「选中文字时显示翻译图标」（默认关）开启后，鼠标抬起经后台取词得到选中文字时在光标旁冒出非激活小图标，点击才以冒图标时的同一份快照唤起翻译浮窗；按下、热键翻译、超时（6s）或再次取词都会作废并收起图标；只读选区，不写剪贴板、不写日志。P4：浮窗对词典结果（kind=dict）复用 DictEntryView 结构化渲染，plain 结果保持纯文本，样式限定在 .mini-translate 作用域。审查后修复：图标窗口 accept_first_mouse、先落盘后改运行时状态、关闭时无论落盘成败先收起图标、取词线程醒来先查代数、6 秒自动收起、热键译文时收起图标、非 macOS 编译的未用导入与函数加 cfg、选中图标窗口独立最小能力集。实跑：cargo test 607 passed、cargo clippy -D warnings 退出 0、cargo fmt --check 退出 0、vitest 85 files / 939 passed、tsc -b 退出 0、vite build 退出 0。真机（首击、焦点、多屏、全屏、CPU、Windows 编译）未验证，已写入 pending-manual.yaml（TP2-M01..M04、TP4-M01）。
evidence:
  commits: []
  files:
    - "src-tauri/src/select_icon.rs（新增：取词调度、代数、图标面板、自动收起、点击翻译）"
    - "src-tauri/src/popover.rs（trigger_popover 拆为 find_spec/show_popover；新增 trigger_translate_prepared；热键译文时收起图标）"
    - "src-tauri/src/settings.rs + src-tauri/src/ipc/settings.rs（select_icon_enabled 字段、get/set 命令、先落盘后改状态）"
    - "src-tauri/src/lib.rs（托管状态、全局鼠标监听、命令注册）"
    - "src-tauri/capabilities/select-icon.json（新增：选中图标窗口最小能力集）"
    - "src/select-icon/（新增入口页、组件、样式、测试）"
    - "src/trans-popover/MiniTranslate.tsx + popover.css + 测试（P4 词典视图）"
    - "src/trans-popover/trans-popover.test.tsx（词典结果复制取摘要的用例）"
    - "src/panels/settings/GeneralPanel.tsx + useGeneralSettings.ts + ipc-client.ts（设置开关）"
    - "vite.config.ts（新增 select-icon 入口）"
  commands:
    - "cd src-tauri && cargo test --lib  # 607 passed / 0 failed"
    - "cd src-tauri && cargo clippy --lib -- -D warnings  # 退出 0"
    - "cd src-tauri && cargo fmt --check  # 退出 0"
    - "npx vitest run  # 85 files / 939 passed"
    - "npx tsc -b  # 退出 0"
    - "npx vite build  # 退出 0"
  urls:
    - "https://developer.apple.com/documentation/appkit/nsevent/1535472-addglobalmonitorforeventsmatchingm"
---

# AI 活动记录 · 翻译交互 P2 选中图标与 P4 词典视图

## 独立审查结论

审查者（未参与实现）结论为「非 BLOCK」，并列出 8 条建议。处理情况如下：

| 审查项 | 处理 |
|---|---|
| 1 图标窗口未开 accept_first_mouse，首击可能被吞 | 已修：`accept_first_mouse(true)` + `focused(false)` |
| 2 写盘失败与运行时状态不一致（隐私开关） | 已修：先落盘成功再改原子量；关闭时无论落盘成败先收起图标 |
| 3 图标只被按下收起，无超时，热键译后仍悬着 | 已修：6 秒自动收起（受代数保护）；热键译文时收起图标与待译内容。键盘/滚动/切应用的全面收起**未做**，见风险 |
| 4 每次抬起一个线程，无合并 | 部分修：线程醒来先查代数，过期即不做 AX 读取。未做单线程合并，见风险 |
| 5 新窗口加进 default capability | 已修：新增 `select-icon.json` 最小能力集，default 未改动 |
| 6 非 macOS 未用导入与函数 | 已修（代码层）：相关导入与函数加 cfg。Windows 完整编译仍受 `ring` 阻塞，未实际验证 |
| 7 默认关闭与坏值回落无断言 | 已修：新增默认值、坏值回落、过期取词判定的测试 |
| 8 P4 复制取摘要无测试 | 已修：新增 dict 复制用例 |

## 风险与未验证项

- 图标首击是否生效、源应用焦点是否真的不被抢、全屏与多屏落点、CPU 与 Chromium/Electron 的 AX 副作用、目标应用卡死时的线程堆积：均需真机，见 TP2-M01..M04。
- 图标不会因键盘输入、滚动、切换应用而收起，只能靠 6 秒超时、鼠标按下或热键翻译收起。
- Windows 编译未验证（`ring` 缺 Windows SDK），按用户要求搁置。
- 浮窗高度 200px 对词典视图偏紧（TP4-M01）。
