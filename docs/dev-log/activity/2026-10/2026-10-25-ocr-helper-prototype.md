---
status: done
category: implementation
subject: Maccy 对标 · Phase 3 P3-9 OCR 阶段 A（独立 Swift Vision helper 原型，不接入 app）
summary: 交付 tools/ocr-helper/ 下可独立复跑的 CLI（ImageIO 读图 + VNRecognizeTextRequest .accurate + 固定键序 JSON），自造 CoreGraphics/CoreText 夹具并逐条断言。行级完全命中 47/62（纯英文 100%、纯中文 75%、混排 75%、密集截图 77%），但字符准确率 96.4%–100%、密集图 99.28%——错例全是「整行只错一个字」（h↔n、全角括号、大小写）。实测暴露两个决定阶段 B 形态的事实：ANE 编译缓存按「可执行文件名」命名空间、首次调用 17–25s（同名第二次 323ms）；revision 3 的置信度只在 0.30/0.50/1.00 三档跳动且正确行也可能是 0.30，不可用于阈值过滤。
evidence:
  commits:
    - "90c3e25 feat: 新增独立 Swift Vision OCR helper 原型与夹具验证（P3-9 阶段 A）"
  commands:
    - "bash tools/ocr-helper/build.sh"
    - "bash tools/ocr-helper/verify.sh"
    - "bash tools/ocr-helper/bench.sh"
    - "swiftc -O -module-cache-path .build-cache/module-cache ocr_helper.swift -o bin/ocr-helper"
  urls: []
---

# AI 活动记录 · P3-9 OCR 阶段 A（Swift Vision helper 独立原型）

## 摘要

- 交付 `tools/ocr-helper/`：`ocr_helper.swift`（helper）、`make_fixtures.swift`（夹具生成器）、
  `build.sh` / `verify.sh` / `bench.sh`、`README.md`。**零改动 `src/`、`src-tauri/`、`Makefile`、
  `Cargo.toml`、`tauri.conf.json`、`package.json`**，不接入 Tauri、不改 IPC、不写库。
- 结论先行：**进程隔离路线成立**（CLI 契约稳定、错误可判、退出码可路由、进程启动仅 ~10ms），
  但**每次调用有 ~250ms 固定识别初始化成本**，且**首次调用要付 17–25s 的 ANE 编译**。
- 识别质量：行级完全命中 47/62，字符准确率 96.4%–100%。**行命中率低是口径严，不是识别崩**——
  所有错例都是整行错一个字，没有漏行、没有串行、没有整段乱码。

## 交付物与契约

| 文件 | 作用 |
|---|---|
| `ocr_helper.swift` | ImageIO 读首帧 → `VNRecognizeTextRequest`（`.accurate`、`zh-Hans/zh-Hant/en-US`、`usesLanguageCorrection=true`）→ stdout 一行 JSON |
| `make_fixtures.swift` | CoreGraphics/CoreText 渲染白底黑字 PNG + 导出真值 `expected.json` |
| `build.sh` | `swiftc -O` 编译两个可执行文件 |
| `verify.sh` | 逐张断言 + 14 项契约/失败路径校验 |
| `bench.sh` | 两档分辨率 × 7 次中位数 |

CLI：`ocr-helper <图片路径>` → `{"ok":true,"text","lines":[{"text","confidence","box":[x,y,w,h]}],"elapsedMs"}`；
失败 `{"ok":false,"error"}` + 非零码（2 参数 / 3 读图 / 4 识别）。

## 三个必须记录的设计取舍

### 1. `box` 保持 Vision 原生左下原点，不翻转

原生语义是归一化 `[0,1]`、**原点左下、y 向上**。没有换算成左上原点是刻意的：换算方向属于下游
渲染/命中框的决策，提前改会让「原生证据」丢失，且下游一旦误以为拿到左上原点，坐标会整体上下颠倒。
README 写明了换算公式 `topY = 1 - (minY + height)`，并注明「未做换算」。

### 2. 手写 JSON 而不用 `JSONEncoder`

契约要求固定键序（`ok`/`text`/`lines`/`elapsedMs`），`JSONEncoder` 不保证顺序。代价是自己承担转义，
因此按 JSON 规范覆盖了全部控制字符，并由 `verify.sh` 用 `python3 json.load` 对**每条**输出做独立校验
（转义写坏会直接红）。

### 3. 只认 PNG/JPEG/TIFF，其它格式明确拒绝

ImageIO 实际能解 HEIC，但阶段 A 只实测过三类容器。HEIC 明确报错（退出码 3、错误里带
`public.heic` 标识）而不是「解出来就往下走」——未实测的像素格式可能带 EXIF 方向或奇怪色深，
静默接受只是把问题推到下游。`verify.sh` 用 `sips` 真转一张 HEIC 来验证这条路径确实会拒绝。

## 实测：命中情况

真值即渲染输入（同一份文本既画进图也写进 `expected.json`），不存在「放宽断言」的空间。
判定口径：期望行在空白归一化后必须出现在识别全文里，不做模糊匹配。

| 图片 | 期望行 | 完全命中 | 行命中率 | 字符错误率 | 字符准确率 |
|---|---|---|---|---|---|
| `en-only.png`（纯英文 34px） | 4 | 4 | **100%** | 0.0% | 100.00% |
| `zh-hans-only.png`（纯中文 34px） | 4 | 3 | 75% | 1.6% | 98.44% |
| `mixed.png`（中英混排 34px） | 4 | 3 | 75% | 2.1% | 97.92% |
| `dense-screenshot.png`（1920×1080，30 行 22px） | 30 | 23 | 77% | 0.7% | **99.28%** |
| `bench-1920x1080.png`（10 行 28px） | 10 | 7 | 70% | 3.1% | 96.91% |
| `bench-3840x2160.png`（同内容 2 倍） | 10 | 7 | 70% | 3.6% | 96.39% |

错字（全部实测，非抽样）分两类，稳定可复现：

1. **h↔n 混淆**是最高频单字错误：`history→nistory`、`Search→Searcn`、`highlight→highlignt`；
   集中在 22–28px 小字号，34px 纯英文 0 错。另有 `启→后`、`窗→阂`、`clipboard→clipboara`
   这类形近替换，以及 `macOS→macos`、`shows→Shows` 大小写错误。
2. **全角/半角括号整段退化**是唯一的大面积错误：`(a) [b] {c}` 读成 `（a） ［b］icr`（相似度 0.77），
   是本批最严重的一行。

## 实测：耗时（Apple M4 Pro / macOS 27.0.1 build 26A434 / arm64 / Swift 6.4）

| 分辨率 | 次数 | `elapsedMs` 中位 | 区间 | 墙钟中位（含进程启动） |
|---|---|---|---|---|
| 1920×1080 | 7 | **409 ms** | 397–419 | 420 ms |
| 3840×2160 | 7 | **426 ms** | 377–602 | 436 ms |

- 两档只差 ~4%：基准图做成同比例 2 倍（像素 ×4、相对字号相同）后耗时基本不变，
  说明框架内部会把输入规整到某个工作分辨率附近——**这是推断，未验证内部实现**；
  可测事实是「像素数翻 4 倍，耗时不变」。
- 墙钟 − `elapsedMs` ≈ 8–10ms，**进程 spawn 不是成本**。
- 补充：4 行小图中位 251–265ms、1080p 10 行 409ms、1080p 30 行 566ms →
  **每次调用约 250ms 起步的固定成本** + 与文字量相关的工作量。
- 跨次运行波动约 ±15%（另一次同跑法得 363ms / 362ms），**单次数字不可下结论**。

## 密集文本截图的定性结论

30 行全部检出、切成 30 个 observation、顺序与原图一致，**无漏行无串行**；1254 字符错 9 个
（准确率 99.28%），7 行各错 1 字。文字足够读懂整屏内容，但会让**精确搜索漏命中**
（例如被读成「阂口」的那条，用户搜「窗口」搜不到）。这条直接影响阶段 B 的匹配语义选择。

## 三个决定阶段 B 形态的实测发现

### 1. ANE 编译缓存按「可执行文件名」命名空间，首次调用 17–25s

`.accurate` 在 Apple Silicon 走 ANE，编译产物缓存在
`~/Library/Caches/<进程名>/com.apple.e5rt.e5bundlecache/<OS build>/...`。

| 场景 | `elapsedMs` |
|---|---|
| 全新 HOME 第一次 | **25542 ms** |
| 同一 HOME 第二次 | 323 ms |
| 换名可执行文件（`ocr-helper-renamed`），同一 HOME | **17031 ms** |
| 改名后第二次 | 361 ms |

即：**helper 必须固定可执行文件名**（改一次名，每个用户再付一次 17–25s），
且首次 OCR 不能同步阻塞——必须异步 + 有状态提示。

### 2. `.accurate` 需要可写 `HOME`，失败时错误毫无信息量

缓存写不进去（本次验收环境是「只允许写工作区」的沙箱）时，`.accurate` 抛
`Foundation._GenericObjCError error 0`，而 `.fast` 反而能跑——极易被误判成「图片有问题」。
为此 `verify.sh` / `bench.sh` 加了 `OCR_SANDBOX_HOME=1` 开关把 `HOME`/`TMPDIR` 指到目录内；
正常开发机不需要。耗时数字都在这个重定向下测，每次测量前预热过缓存。

### 3. 置信度不可用于阈值过滤

revision 3 的 `confidence` 只在 `0.30 / 0.50 / 1.00` 三档跳动，**完全正确的行也可能是 0.30**
（`OCR 识别 test 2026 年10 月` 置信度 0.30）；revision 2 对同样内容给 1.00，识别文本完全相同。
阶段 B 若按置信度丢弃低分行，会丢掉大量正确结果。

顺带确认一条参数敏感点：**语言顺序是承重墙**。`["zh-Hans","en-US"]` 中文正确；把 `en-US`
放首位，中文整段退化成乱码（`复制一次随处粘贴` → `#I-BELLE`）。框架按首个语言选模型。

## 未验证项

多语言回退（只测契约三语言）、旋转/竖排文本（`cgImage` 路径不处理 EXIF 方向）、
10–14px 极小字号、真实截图（深色模式/渐变背景/Retina 次像素）、大图内存与超时（最大只到 4K）、
多 helper 进程并发下 ANE 是否互相拖慢、PDF/动图/多帧 TIFF、Windows（Vision 不适用）。

## 建议 lead 写入 P3-9 的要点

阶段 A 通过：进程隔离路线成立，**不需要**先做常驻进程；但接入时必须
（a）异步 + 首次识别状态提示、（b）固定 helper 可执行文件名、（c）不要把置信度当阈值、
（d）中文语言必须排在英文前、（e）明确 OCR 文本的匹配语义（精确 vs 模糊）以兜住全角括号与 h↔n 这类单字错误。
