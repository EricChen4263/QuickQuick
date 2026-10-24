# OCR helper（Swift + Vision 独立原型）

P3-9 阶段 A 的交付物：一个可独立复跑的 OCR 命令行工具，用来回答「用 Swift helper + IPC 给 QuickQuick
补图片文字识别，识别质量和耗时是否可接受」。

**这不是产品代码**：不接入 Tauri、不改 IPC、不写数据库、不碰 `src/` `src-tauri/` `Makefile`
`Cargo.toml` `tauri.conf.json` `package.json`。等阶段 B 决定接入方式后再动应用侧。

## 文件

| 文件 | 作用 |
|---|---|
| `ocr_helper.swift` | helper 主体（ImageIO 读图 + `VNRecognizeTextRequest` 识别 + JSON 输出） |
| `make_fixtures.swift` | 夹具生成器：CoreGraphics/CoreText 把已知文本渲染成 PNG，并导出真值 `expected.json` |
| `build.sh` | 编译上面两个可执行文件到 `bin/` |
| `verify.sh` | 逐张夹具断言 + CLI 契约与失败路径校验 |
| `bench.sh` | 两档分辨率耗时基准（中位数） |

`bin/`、`fixtures/`、`.build-cache/` 都是派生物，已 `.gitignore`。

## 构建

```bash
cd tools/ocr-helper
bash build.sh
```

产物：`bin/ocr-helper`、`bin/make-fixtures`。等价的手写命令：

```bash
swiftc -O -module-cache-path .build-cache/module-cache ocr_helper.swift -o bin/ocr-helper
swiftc -O -module-cache-path .build-cache/module-cache make_fixtures.swift -o bin/make-fixtures
```

- 要求 macOS 13+（`VNRecognizeTextRequest.supportedRecognitionLanguages()` 实例方法的下限），
  本机实测 macOS 27.0.1 / Swift 6.4 / arm64。
- 模块缓存与 `TMPDIR` 都指到 `.build-cache/`：`swiftc` 默认写 `$TMPDIR/clang/ModuleCache`，
  在「只允许写工作区」的受限环境里会直接报 `Operation not permitted`。这样构建不依赖外部可写目录。

## 运行

```bash
./bin/ocr-helper <图片路径>
```

### CLI 契约

stdout 固定一行 JSON：

```json
{"ok":true,"text":"<全文>","lines":[{"text":"...","confidence":0.5,"box":[0.0333,0.875,0.5625,0.0583]}],"elapsedMs":301}
```

失败时：

```json
{"ok":false,"error":"文件不存在：/path/to/nope.png"}
```

- 键序固定为 `ok` / `text` / `lines` / `elapsedMs`（手写 JSON，不用 `JSONEncoder`，因为它不保证顺序；
  字符串转义按 JSON 规范覆盖全部控制字符）。
- `text` 为全部行按阅读顺序用 `\n` 连接的结果。
- `lines[].text` 是单行原文，`lines[].confidence` 是该行 top-1 候选的置信度，`lines[].box` 见下节。
- `elapsedMs` 覆盖「解码 + 识别」，**不含进程启动与框架加载**；进程启动成本由调用方测墙钟体现
  （实测差值约 8–10ms，可忽略）。
- 只接受一个位置参数；多参数视为用法错误。

### `box` 的坐标系（重要）

**保持 Vision 原生语义，未做任何换算**：归一化到 `[0,1]`，**原点在左下角，y 轴向上**。
即 `y = 0` 是图片底边，`y = 1` 是顶边；`box = [minX, minY, width, height]`。

没有翻转成「左上原点」是刻意的：换算方向属于下游渲染/命中框的决策，提前改会让「原生证据」丢失，
且一旦下游以为拿到的是左上原点，坐标会整体上下颠倒。要转成左上原点公式：

```text
topY = 1 - (minY + height)
```

### 阅读顺序

Vision 的观察顺序不保证版面顺序，helper 统一按「上到下、左到右」重排：先按 `maxY` 降序
（左下原点，故 `maxY` 大的在上），`maxY` 相差 ≤0.01 视为同一行，再按 `minX` 升序。
只做粗粒度版面恢复，**不合并同一行的多段文本**（Vision 对同一行里相隔较远的片段会给出多个 observation）。

### 退出码

| 码 | 含义 |
|---|---|
| 0 | 成功 |
| 2 | 参数错误（缺参数、未知选项、多余参数） |
| 3 | 读图失败（文件不存在、无法打开、类型不支持、解码失败） |
| 4 | 识别失败（Vision `perform` 抛错、能力查询失败） |

失败一律输出 `{"ok":false,"error":"..."}` 到 stdout 并以非零码退出，**不静默吞错**：
错误信息带路径、ImageIO 类型标识（如 `public.heic`）或 Vision 原始描述。

### 图片格式

经 ImageIO（`CGImageSource`）解码首帧，支持 `public.png` / `public.jpeg` / `public.tiff`。
其它类型（例如 HEIC）**明确拒绝并报错**（退出码 3，错误里带检测到的类型标识），
而不是「能解出来就往下走」——未实测的像素格式可能带 EXIF 方向或奇怪色深，静默接受只会把问题推到下游。

### 诊断

```bash
./bin/ocr-helper --capabilities   # 本机 Vision 支持的 revision 与语言清单（JSON）
./bin/ocr-helper --help           # 人类可读用法（非 JSON）
```

`--capabilities` 用来分辨「识别失败是因为本机缺某个语言」还是「图有问题」——契约固定请求
`zh-Hans` / `zh-Hant` / `en-US`，其中任一不被支持时 `perform()` 会抛错。本机支持 33 种语言且三种都在列。

### 识别参数（契约固定）

```swift
recognitionLevel = .accurate
recognitionLanguages = ["zh-Hans", "zh-Hant", "en-US"]
usesLanguageCorrection = true
```

实测参数敏感性（`fixtures/mixed.png`，同一台机器）：

- **语言顺序是承重墙**：`["zh-Hans","en-US"]` 得到完整正确的中文行；把 `en-US` 放到首位，
  中文段落直接退化成乱码（`QuickQuick EFIE` / `复制一次随处粘贴` → `#I-BELLE`）。
  框架按首个语言选模型，中文必须排在英文前面。
- `.fast` 完全不可用（整段变乱码）；`.accurate` 是唯一可用档位。
- `usesLanguageCorrection` 开/关在本批夹具上输出完全一致。
- `revision` 1/2/3 识别文本相同，**但置信度不同**：revision 3（当前）在同样正确的行上给 0.50/0.30，
  revision 2 给 1.00。见下节。

## 验证

三个脚本都可独立复跑，默认在当前目录内产出派生物：

```bash
bash verify.sh    # 生成夹具 → 逐张断言 → 契约与失败路径校验
bash bench.sh     # 两档分辨率 × 7 次取中位数
RUNS=10 bash bench.sh
```

### 夹具怎么来的

`make_fixtures.swift` 用 CoreGraphics/CoreText 渲染白底黑字的 PNG，字体 `Helvetica`（纯英文）
与 `PingFangSC-Regular`（中文、混排）。**真值即渲染输入**，同时导出到 `fixtures/expected.json`，
所以「期望文本」与「实际像素」不可能漂移——改了真值就是改了图上写的字，断言会跟着变，不是放宽断言。

生成的 6 张图：

| 文件 | 尺寸 | 字号 | 内容 |
|---|---|---|---|
| `en-only.png` | 1200×600 | 34 | 4 行纯英文（含日期、数字） |
| `zh-hans-only.png` | 1200×600 | 34 | 4 行纯简体中文 |
| `mixed.png` | 1400×700 | 34 | 4 行中英混排（同行内切换语言） |
| `dense-screenshot.png` | 1920×1080 | 22 | 30 行「一整屏文档」式密集中英混排 |
| `bench-1920x1080.png` | 1920×1080 | 28 | 10 行混排（性能基准） |
| `bench-3840x2160.png` | 3840×2160 | 56 | 同一内容的 2 倍版（性能基准） |

生成器自带两条硬校验，避免「夹具本身就坏」被误当成识别问题：单行超出画布宽度、
内容超出画布高度，都直接报错而不是裁掉。基准两档用**同一份内容 + 同比例的字号/边距**（2 倍版
字号与边距都是 2 倍），所以两档差异只来自像素数量。

### `verify.sh` 的断言口径与退出码

- 命中判定：期望行在「空白归一化」后必须出现在识别全文里。**只归一化空白**（OCR 会在句中断行），
  不做模糊/容错匹配，字符必须对得上。
- 另算**字符错误率**（`(替换+删除+插入)/期望字符数`，`difflib` 匹配块反推）：行级命中率看不出
  「只差一个字」，这个指标才反映识别完整度。
- 未命中时打印期望行、最接近的识别行（相似度 + 置信度），即真实错字。
- 退出码：`0` 全绿 / `1` 契约或失败路径校验有问题（脚本或 helper 的缺陷）/ `2` 仅识别命中不足
  （已知质量缺口，不等于脚本有 bug）。本机实测是 `2`。

## 实测结果

机器：**Apple M4 Pro**（`sysctl -n machdep.cpu.brand_string`），**macOS 27.0.1（26A434）**，
arm64，Swift 6.4（`swift-driver 1.168.6`）。所有数字都来自本机实跑。

### 1. 命中情况（`bash verify.sh`）

| 图片 | 尺寸 | 期望行 | 完全命中行 | 行命中率 | 字符错误率 |
|---|---|---|---|---|---|
| `en-only.png` | 1200×600 | 4 | 4 | **100%** | 0.0% |
| `zh-hans-only.png` | 1200×600 | 4 | 3 | **75%** | 1.6% |
| `mixed.png` | 1400×700 | 4 | 3 | **75%** | 2.1% |
| `dense-screenshot.png` | 1920×1080 | 30 | 23 | **77%** | 0.7% |
| `bench-1920x1080.png` | 1920×1080 | 10 | 7 | **70%** | 3.1% |
| `bench-3840x2160.png` | 3840×2160 | 10 | 7 | **70%** | 3.6% |
| 合计 | — | 62 | 47 | 76% | — |

字符级准确率（期望字符数 / 编辑距离）：

| 图片 | 期望字符 | 编辑距离 | 字符准确率 |
|---|---|---|---|
| `en-only.png` | 180 | 0 | **100.00%** |
| `zh-hans-only.png` | 64 | 1 | 98.44% |
| `mixed.png` | 96 | 2 | 97.92% |
| `dense-screenshot.png` | 1254 | 9 | **99.28%** |
| `bench-1920x1080.png` | 388 | 12 | 96.91% |
| `bench-3840x2160.png` | 388 | 14 | 96.39% |

**行命中率低但字符准确率高**是这批数据的核心特征：所有错例都是「整行只错 1 个字」，
没有一行丢失、没有串行、没有整段乱码。逐条错字（全部实测，不是抽样）：

| 图片 | 期望 | 实测 | 差异 |
|---|---|---|---|
| `zh-hans-only` | 光学字符识别把图片里**的**文字变成可搜索文本 | 光学字符识别把图片里文字变成可搜索文本 | 删 1 字 |
| `mixed` | Copy once, paste anywhere 复制一次随**处**粘贴 | Copy once, paste anywhere 复制一次随粘贴 | 删 1 字 |
| `mixed` | OCR 识别 test 2026 年**␣**10 月 | OCR 识别 test 2026 **年10** 月 | 空格位置 |
| `dense` | ... out of the **h**istory temporarily. | ... out of the **n**istory temporarily. | h→n |
| `dense` | ... 不会阻塞**启**动 ... | ... 不会阻塞**后**动 ... | 启→后 |
| `dense` | ... never contain clipboa**rd** text. | ... never contain clipboa**ra** text. | rd→ra |
| `dense` | 支持 **macOS** 13 及以上版本 ... | 支持 **macos** 13 及以上版本 ... | 大小写 |
| `dense` | The status menu **s**hows ... | The status menu **S**hows ... | 大小写 |
| `dense` | **Searc**h results hig**h**light ... | **Searcn** results hig**h**lig**n**t ... | h→n ×2 |
| `dense` | 复制图片后可以直接粘贴到聊天**窗**口 ... | 复制图片后可以直接粘贴到聊天**阂**口 ... | 窗→阂 |
| `bench-1920` / `bench-3840` | 第七行包含数字 2026 与符号 **(a) [b] {c}**。 | ... 与符号 **（a） ［b］icr**。 | 全角化 + 右半段崩坏 |
| `bench-1920` / `bench-3840` | ... covered by **glyphs** ... | ... covered by **giyphs** / **glyphs** ... | l→i（仅 1080 档） |
| `bench-3840` | 第四行：图片里**的**文字 ... | 第四行：图片里文字 ... | 删 1 字 |

错字分布有两个稳定的规律，对阶段 B 有用：

1. **h↔n 混淆**是最高频的单字错误（`history→nistory`、`Search→Searcn`、`highlight→highlignt`），
   集中在 22–28px 小字号；34px 的纯英文夹具 0 错。
2. **全角/半角括号整段退化**是唯一的大面积错误：`(a) [b] {c}` 被读成全角并把右半段读成 `icr`，
   这是本批最严重的单行（相似度 0.77）。混排符号（括号、花括号）是已知薄弱点。

### 2. 耗时（`bash bench.sh`，每档 7 次取中位数）

| 分辨率 | 次数 | `elapsedMs` 中位数 | `elapsedMs` 区间 | 墙钟中位数（含进程启动） | 识别行数 |
|---|---|---|---|---|---|
| 1920×1080 | 7 | **409 ms** | 397–419 | 420 ms | 10 |
| 3840×2160 | 7 | **426 ms** | 377–602 | 436 ms | 10 |

- 两档只差 ~4%，**4K 没有明显更贵**。把基准图做成同比例 2 倍（相同内容、相同相对字号）后，
  4 倍像素数几乎不增加耗时，说明框架内部会把输入规整到某个工作分辨率附近——这是**推断**，
  未验证其内部实现；可测事实是「像素数翻 4 倍，耗时基本不变」。
- 跨次运行波动约 ±15%（另一次同样跑法得到 1920×1080 中位 363ms、4K 中位 362ms），
  所以**不要用单次数字下结论**，要报中位数与区间。
- 墙钟 − `elapsedMs` ≈ 8–10ms，即进程 spawn 成本可忽略，**固定成本在识别本身**。

补充实测（非验收要求，用于判断固定成本）：4 行小图（1200×600 / 1400×700）中位 251–265ms，
1080p 10 行 409ms，1080p 30 行密集图 566ms。即：**每次调用约 250ms 起步的固定成本**
（框架/模型加载，与图片大小无关），再叠加与文字量相关的工作量。

### 3. 密集文本截图的定性结论（`dense-screenshot.png`，1920×1080，30 行）

- 耗时 556–801ms（多次实测区间），约为 10 行图的两倍，与行数近似线性。
- **版面完整**：30 行全部被检出、切分成 30 个 observation，顺序与原图一致，没有漏行、没有串行。
- **文字完整度高**：字符准确率 99.28%（1254 字符错 9 个），7 行各错 1 字，其余 23 行逐字正确。
- 错字全部是形近替换或大小写，不影响「这屏图讲的是什么」的可读性，但**会让精确搜索漏命中**——
  例如用户搜「窗口」时，那条被读成「阂口」的记录搜不到。这正是阶段 B 需要决定的语义：
  图片的 OCR 文本进 `title` 还是单独的 `ocr_text` 列、以及匹配用精确还是模糊。

## 环境注意事项（实测踩到的）

### 首次调用要付一次神经引擎编译成本

`VNRecognizeTextRequest` 的 `.accurate` 档在 Apple Silicon 上走 ANE，**模型编译结果缓存在
`~/Library/Caches/<进程名>/com.apple.e5rt.e5bundlecache/<OS build>/...`**。实测：

| 场景 | `elapsedMs` |
|---|---|
| 全新 HOME（无缓存）第一次调用 | **25542 ms** |
| 同一 HOME 第二次调用 | 323 ms |
| 换一个可执行文件名（`ocr-helper-renamed`），同一 HOME | **17031 ms** |
| 改名后第二次调用 | 361 ms |

两个结论：

1. **缓存命名空间是可执行文件名**，不是静态库或框架。阶段 B 的 helper 必须用一个**固定名字**，
   改名 / 每次构建换名会让每个用户再多付一次 17–25s。开发期同名重建会复用缓存（名字没变）。
2. 首次 17–25s 是不可接受的用户等待，接入时必须异步 + 有进度或「正在识别」状态，
   且不能阻塞列表查询。

### `.accurate` 需要可写 HOME

缓存写不进去时，`.accurate` 会直接失败并抛一个**毫无信息量**的通用错误
（`Foundation._GenericObjCError error 0`），`.fast` 反而能跑——这个失败模式极易被误判为「图片有问题」。
本次验收环境是「只允许写工作区」的沙箱，`HOME` 不可写，所以 `verify.sh` / `bench.sh` 支持
`OCR_SANDBOX_HOME=1` 把 `HOME` / `TMPDIR` 指到 `.build-cache/` 内。**正常开发机不需要这个开关**
（用户的 `HOME` 本来就可写）。上面的耗时数字都是在这个重定向下测的；缓存预热过一次后
与用户目录无差别，且每次测量前都跑预热。

### 置信度不可用于阈值判定

当前 revision 3 的 `confidence` 只出现 `0.30 / 0.50 / 1.00` 三个值，且**正确的行也可能是 0.30**
（例如 `OCR 识别 test 2026 年10 月` 置信度 0.30 而文字基本正确）。revision 2 对同样内容给 1.00。
所以阶段 B **不要**拿置信度做「低于阈值就丢弃」的判定，否则会丢掉大量正确结果。

## 未验证项

- **多语言回退**：只验证了契约固定的三语言组合。其它语言（日/韩/俄等，本机支持 33 种）未测；
  「图片里只有日文时会不会退化」未测。
- **旋转文本**：`VNImageRequestHandler(cgImage:)` 默认 `.up`，不处理 EXIF 方向，也不做旋转检测。
  竖排中文、90°/180° 旋转文本全部未测。
- **极小字号**：最小只测到 22px（密集图）。10–14px 的真实截图小字未测，h↔n 混淆在更小字号下
  大概率更严重。
- **真实截图**：夹具是合成的白底黑字。深色模式、渐变背景、半透明、图标叠加、抗锯齿/次像素渲染、
  Retina 缩放等真实场景均未测。
- **Windows**：不适用（Vision 是 Apple 框架）。
- **大图内存与超时**：最大只测到 3840×2160。更长截图（整页网页、多屏拼接）的峰值内存与是否超时未测。
- **进程并发**：多个 helper 同时跑（例如用户连拍多张图）时 ANE 是否会串行/互相拖慢未测。
- **PDF/动图/多帧 TIFF**：只读首帧，多页文档未验证。
- **`types()` 之外的图片容器**（WebP、HEIC）：当前明确拒绝，未做「支持」验证。

## 阶段 B 建议（供裁决）

1. **接入形态**：进程隔离成立——CLI 契约稳定、错误可判、退出码可路由；`elapsedMs` 与墙钟差 ~10ms，
   说明每个进程的启动成本不是瓶颈，**真正的固定成本是 ~250ms 的识别初始化**。
   所以「每次 OCR 起一个进程」可行，但**不要在主线程/列表查询路径上同步调用**。
2. **长期形态**：若单次 OCR 要频繁触发（图片批量导入），再考虑常驻进程 + IPC，
   用一次进程换掉每次 250ms 的固定开销。阶段 A 的数据足够做这个判断，无需先做常驻。
3. **首次调用**：必须异步 + 明确状态；`~/Library/Caches/<helper 名>/` 要允许应用写（非沙盒 app 天然允许）。
4. **`box` 落库**：保留原生左下原点语义，前端渲染时按 `topY = 1 - (minY + height)` 换算，
   避免两套坐标系来回转。
5. **质量兜底**：全角括号、h↔n、大小写三类错误决定「搜图片」该用精确还是模糊匹配；
   置信度不可用于过滤（见上）。
