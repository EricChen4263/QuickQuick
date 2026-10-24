// 生成 OCR 验证夹具：用 CoreGraphics/CoreText 把已知文本渲染成白底黑字 PNG。
//
// 为什么自造图片而不是拿网络截图：断言必须锚定「图里到底写了什么」，外部截图的真值不可控，
// 改了真值就等于放宽断言。这里的文本既是渲染输入也是 expected.json 的唯一来源，
// 所以「期望文本」与「实际像素」不可能漂移。

import CoreGraphics
import CoreText
import Foundation
import ImageIO
import UniformTypeIdentifiers

struct Fixture {
    let fileName: String
    let width: Int
    let height: Int
    let fontName: String
    let fontSize: CGFloat
    let lines: [String]
    /// 行距倍数（相对字号）。密集文本用更小的行距才能在固定画布内塞满，同时保持版面真实。
    let lineHeightFactor: CGFloat

    init(
        fileName: String,
        width: Int,
        height: Int,
        fontName: String,
        fontSize: CGFloat,
        lines: [String],
        lineHeightFactor: CGFloat = 1.75
    ) {
        self.fileName = fileName
        self.width = width
        self.height = height
        self.fontName = fontName
        self.fontSize = fontSize
        self.lines = lines
        self.lineHeightFactor = lineHeightFactor
    }
}

struct GeneratorError: Error {
    let message: String
}

/// 纯英文夹具：验证拉丁文字路径（含数字与标点），用 Helvetica 与中文夹具区分字体。
let englishLines = [
    "The quick brown fox jumps over the lazy dog",
    "Clipboard history makes copy and paste faster",
    "Optical character recognition runs on device",
    "Version 2.6 ships on 2026-10-25 with 12 fixes",
]

/// 纯简体中文夹具。
let simplifiedChineseLines = [
    "剪贴板历史记录可以提高工作效率",
    "光学字符识别把图片里的文字变成可搜索文本",
    "图片中的中文也能被检索到",
    "快捷键唤起面板后可以直接粘贴",
]

/// 中英混排夹具：验证同一行内中英切换不会互相吞字。
let mixedLines = [
    "QuickQuick 剪贴板工具",
    "Copy once, paste anywhere 复制一次随处粘贴",
    "OCR 识别 test 2026 年 10 月",
    "搜索 Search 支持英文与中文关键词",
]

/// 截图式密集文本：模拟一整屏文档/网页，用于定性观察识别完整度（漏词、断行、错字）。
let denseLines = [
    "QuickQuick 使用说明",
    "本工具会在后台监听剪贴板变化，把每一次复制记录下来。",
    "按下快捷键可以立刻唤起面板，用键盘选择后直接粘贴。",
    "The panel remembers your last search and restores it on reopen.",
    "所有条目都保存在本地数据库中，不会上传到任何服务器。",
    "Settings 页面可以调整最大条数、图片高度与预览延迟。",
    "图片条目默认只在列表里显示缩略图，点击才展开原始尺寸。",
    "You can pin frequently used items so they stay at the top.",
    "搜索框支持精确匹配、模糊匹配与正则表达式三种模式。",
    "Paused capture keeps new copies out of the history temporarily.",
    "清理策略：先删最旧的未固定条目，固定条目永不自动删除。",
    "The app writes a private pasteboard marker to skip self-copies.",
    "翻译功能需要先在设置里配置服务商与密钥。",
    "Portable export packs the database with a password you choose.",
    "更新检查失败时不会阻塞启动，只会在设置页提示重试。",
    "Each image thumbnail is stored as WebP with a 256 pixel edge.",
    "如果快捷键与其他应用冲突，可以在设置里换成另一组组合键。",
    "The panel is pinned to the cursor screen when multiple displays exist.",
    "导入备份会在新库上重建索引，旧库保持不动。",
    "Logs are written only on failure and never contain clipboard text.",
    "支持 macOS 13 及以上版本，Windows 版本仍在验证中。",
    "The status menu shows the current capture state as an icon.",
    "卸载前请先导出备份，否则加密后的数据库无法直接读取。",
    "热键冲突检测会在首次启动时给出提示。",
    "Search results highlight the matched substring in the title.",
    "面板高度会随结果数量自适应，最多显示十行。",
    "Failed translations surface the provider error message verbatim.",
    "复制图片后可以直接粘贴到聊天窗口，格式保持原始 PNG。",
    "The database file lives under the app support directory.",
    "Every list query orders by last modified time then row id.",
]

/// 两档分辨率用同一份内容与同一套版面比例，只把 fontSize/margin 整体乘以 scale，
/// 这样耗时差异来自像素数量而不是「内容变了」。
let benchLines = [
    "QuickQuick 性能基准文字样本",
    "The quick brown fox jumps over the lazy dog 0123456789",
    "剪贴板历史记录中英文混排，用于测量整图识别耗时。",
    "Vision performs accurate text recognition on device.",
    "第四行：图片里的文字应当被完整识别出来。",
    "Line six keeps the density close to a real screenshot.",
    "第七行包含数字 2026 与符号 (a) [b] {c}。",
    "Two thirds of this image is covered by glyphs, not blank space.",
    "第九行：用于验证跨行断句不会吞掉字符。",
    "Closing line with mixed 中文 and English words.",
]

func makeFixture(_ fixture: Fixture) throws {
    let colorSpace = CGColorSpaceCreateDeviceRGB()
    let bitmapInfo = CGImageAlphaInfo.premultipliedLast.rawValue
    guard
        let context = CGContext(
            data: nil,
            width: fixture.width,
            height: fixture.height,
            bitsPerComponent: 8,
            bytesPerRow: 0,
            space: colorSpace,
            bitmapInfo: bitmapInfo
        )
    else {
        throw GeneratorError(message: "无法创建 \(fixture.fileName) 的位图上下文")
    }

    context.setFillColor(red: 1, green: 1, blue: 1, alpha: 1)
    context.fill(CGRect(x: 0, y: 0, width: fixture.width, height: fixture.height))
    context.setAllowsAntialiasing(true)
    context.setShouldAntialias(true)
    context.setShouldSmoothFonts(false)

    let font = CTFontCreateWithName(fixture.fontName as CFString, fixture.fontSize, nil)
    let resolvedName = CTFontCopyPostScriptName(font) as String
    if resolvedName != fixture.fontName {
        emitDiagnostic(
            "警告：请求字体 \(fixture.fontName) 不可用，\(fixture.fileName) 实际使用 \(resolvedName)"
        )
    }

    let margin = fixture.fontSize * 1.2
    let lineHeight = fixture.fontSize * fixture.lineHeightFactor
    let ascent = CTFontGetAscent(font)
    let descent = CTFontGetDescent(font)
    let usableWidth = CGFloat(fixture.width) - margin * 2
    let attributes: [CFString: Any] = [
        kCTFontAttributeName: font,
        kCTForegroundColorAttributeName: CGColor(red: 0, green: 0, blue: 0, alpha: 1),
    ]

    for (index, text) in fixture.lines.enumerated() {
        guard
            let attributed = CFAttributedStringCreate(nil, text as CFString, attributes as CFDictionary)
        else {
            throw GeneratorError(message: "无法为 \(fixture.fileName) 第 \(index + 1) 行创建属性字符串")
        }
        let line = CTLineCreateWithAttributedString(attributed)
        let lineWidth = CGFloat(CTLineGetTypographicBounds(line, nil, nil, nil))
        if lineWidth > usableWidth {
            throw GeneratorError(
                message: "\(fixture.fileName) 第 \(index + 1) 行超出画布宽度"
                    + "（\(Int(lineWidth)) > \(Int(usableWidth))），换行或缩小字号后重试"
            )
        }
        let baselineY = CGFloat(fixture.height) - margin - ascent - CGFloat(index) * lineHeight
        if baselineY - descent < 0 {
            throw GeneratorError(message: "\(fixture.fileName) 内容超出画布高度，第 \(index + 1) 行已越界")
        }
        // 基线对齐到整数像素：小数位置的文字会被抗锯齿轻微模糊，夹具本身不该带这种噪声。
        context.textPosition = CGPoint(x: margin.rounded(), y: baselineY.rounded())
        CTLineDraw(line, context)
    }

    guard let image = context.makeImage() else {
        throw GeneratorError(message: "无法从上下文导出 \(fixture.fileName) 的图像")
    }
    try writePNG(image, fileName: fixture.fileName)
}

func writePNG(_ image: CGImage, fileName: String) throws {
    let url = outputDirectory.appendingPathComponent(fileName)
    guard
        let destination = CGImageDestinationCreateWithURL(
            url as CFURL,
            UTType.png.identifier as CFString,
            1,
            nil
        )
    else {
        throw GeneratorError(message: "无法创建 \(fileName) 的 PNG 输出目标：\(url.path)")
    }
    CGImageDestinationAddImage(destination, image, nil)
    guard CGImageDestinationFinalize(destination) else {
        throw GeneratorError(message: "PNG 写入失败：\(url.path)")
    }
    emitDiagnostic("已生成 \(url.path)（\(image.width)x\(image.height)）")
}

/// 期望文本与图片同源，故直接由夹具定义导出，供 verify 脚本读取断言。
func writeExpectedJSON(_ fixtures: [Fixture]) throws {
    let payload: [String: Any] = [
        "fixtures": fixtures.map { fixture -> [String: Any] in
            [
                "image": fixture.fileName,
                "width": fixture.width,
                "height": fixture.height,
                "fontName": fixture.fontName,
                "fontSize": Double(fixture.fontSize),
                "lines": fixture.lines,
            ]
        }
    ]
    let data = try JSONSerialization.data(withJSONObject: payload, options: [.prettyPrinted, .sortedKeys])
    let url = outputDirectory.appendingPathComponent("expected.json")
    try data.write(to: url)
    emitDiagnostic("已生成 \(url.path)")
}

func emitDiagnostic(_ line: String) {
    guard let data = (line + "\n").data(using: .utf8) else {
        return
    }
    FileHandle.standardError.write(data)
}

let arguments = Array(CommandLine.arguments.dropFirst())
let outputDirectory = arguments.first.map { URL(fileURLWithPath: $0) } ?? URL(fileURLWithPath: "fixtures")

let fixtures: [Fixture] = [
    Fixture(
        fileName: "en-only.png",
        width: 1200,
        height: 600,
        fontName: "Helvetica",
        fontSize: 34,
        lines: englishLines
    ),
    Fixture(
        fileName: "zh-hans-only.png",
        width: 1200,
        height: 600,
        fontName: "PingFangSC-Regular",
        fontSize: 34,
        lines: simplifiedChineseLines
    ),
    Fixture(
        fileName: "mixed.png",
        width: 1400,
        height: 700,
        fontName: "PingFangSC-Regular",
        fontSize: 34,
        lines: mixedLines
    ),
    Fixture(
        fileName: "dense-screenshot.png",
        width: 1920,
        height: 1080,
        fontName: "PingFangSC-Regular",
        fontSize: 22,
        lines: denseLines,
        lineHeightFactor: 1.5
    ),
    Fixture(
        fileName: "bench-1920x1080.png",
        width: 1920,
        height: 1080,
        fontName: "PingFangSC-Regular",
        fontSize: 28,
        lines: benchLines
    ),
    Fixture(
        fileName: "bench-3840x2160.png",
        width: 3840,
        height: 2160,
        fontName: "PingFangSC-Regular",
        fontSize: 56,
        lines: benchLines
    ),
]

do {
    try FileManager.default.createDirectory(at: outputDirectory, withIntermediateDirectories: true)
    for fixture in fixtures {
        try makeFixture(fixture)
    }
    try writeExpectedJSON(fixtures)
    exit(0)
} catch let error as GeneratorError {
    emitDiagnostic("生成失败：\(error.message)")
    exit(1)
} catch {
    emitDiagnostic("生成失败：\(error)")
    exit(1)
}
