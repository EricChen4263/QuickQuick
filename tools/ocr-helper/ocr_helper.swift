// QuickQuick OCR helper（阶段 A 独立原型，不接入 app）。
//
// 单一职责：把一张图片交给 Vision 做文字识别，把结果按固定 CLI 契约写到 stdout。
// 不读环境变量、不写磁盘、不联网、不持有任何跨调用状态，因此可以独立复跑。

import CoreGraphics
import Foundation
import ImageIO
import Vision

/// 阶段 A 只验证这三类图片。其它类型（如 HEIC）明确报错，而不是「解出来什么算什么」：
/// 未经实测的像素格式可能带 EXIF 方向、色深或 alpha 语义，静默接受只会把问题推到下游。
private let supportedImageTypes: Set<String> = ["public.png", "public.jpeg", "public.tiff"]

/// 识别语言按契约固定，不随系统语言或图片内容变化，保证同一张图在多次运行间可比较。
private let requestedLanguages = ["zh-Hans", "zh-Hant", "en-US"]

/// 退出码区分失败原因，避免调用方只能看到「非零」而无法判断该重试还是该换图。
enum HelperExitCode: Int32 {
    case success = 0
    case usage = 2
    case imageRead = 3
    case recognition = 4
}

/// 错误必须带上足以定位的上下文（路径、类型标识、Vision 原始描述），不吞任何失败。
struct HelperError: Error {
    let code: HelperExitCode
    let message: String
}

enum Command {
    case recognize(path: String)
    case help
    case capabilities
}

/// 一行识别结果。`box` 保持 Vision 原生语义：归一化坐标、原点在左下角、y 轴向上。
/// 此处不做翻转——换算方向是下游的决策，提前改会让「原生证据」丢失。
struct RecognizedLine {
    let text: String
    let confidence: Float
    let box: CGRect
}

let usageText = """
用法：ocr-helper <图片路径>
      ocr-helper --capabilities
      ocr-helper --help

识别成功时向 stdout 输出一行 JSON：
  {"ok":true,"text":"<全文>","lines":[{"text":"...","confidence":0.0,"box":[x,y,w,h]}],"elapsedMs":123}
失败时向 stdout 输出 {"ok":false,"error":"<原因>"} 并以非零码退出：
  2 参数错误 / 3 读图失败 / 4 识别失败

box 为 Vision 原生归一化坐标（[0,1]，原点左下角，y 轴向上）。
"""

func parseCommand(_ arguments: [String]) throws -> Command {
    guard let first = arguments.first else {
        throw HelperError(code: .usage, message: "缺少图片路径参数")
    }
    switch first {
    case "--help", "-h":
        return .help
    case "--capabilities":
        return .capabilities
    default:
        guard !first.hasPrefix("-") else {
            throw HelperError(code: .usage, message: "未知选项：\(first)")
        }
        guard arguments.count == 1 else {
            throw HelperError(code: .usage, message: "只接受一个图片路径，实际收到 \(arguments.count) 个参数")
        }
        return .recognize(path: first)
    }
}

/// 经 ImageIO 解码首帧。用 CGImageSource 而不是 NSImage，是因为后续 Vision 需要 CGImage，
/// 且 CGImageSource 能报出真实的容器类型标识用于错误信息。
func loadImage(at path: String) throws -> CGImage {
    let url = URL(fileURLWithPath: path)
    guard FileManager.default.fileExists(atPath: url.path) else {
        throw HelperError(code: .imageRead, message: "文件不存在：\(path)")
    }
    let options = [kCGImageSourceShouldCache: false] as CFDictionary
    guard let source = CGImageSourceCreateWithURL(url as CFURL, options) else {
        throw HelperError(code: .imageRead, message: "无法打开图片源：\(path)")
    }
    let typeIdentifier = CGImageSourceGetType(source).map { $0 as String }
    if let typeIdentifier, !supportedImageTypes.contains(typeIdentifier) {
        let supported = supportedImageTypes.sorted().joined(separator: ", ")
        throw HelperError(
            code: .imageRead,
            message: "不支持的图片类型 \(typeIdentifier)，当前只支持 \(supported)：\(path)"
        )
    }
    guard CGImageSourceGetCount(source) > 0 else {
        throw HelperError(code: .imageRead, message: "图片源内没有可解码的帧：\(path)")
    }
    guard let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else {
        let type = typeIdentifier ?? "unknown"
        throw HelperError(code: .imageRead, message: "解码失败（类型 \(type)）：\(path)")
    }
    return image
}

/// Vision 返回的观察顺序不保证阅读顺序，这里按「上到下、左到右」重排（左下原点故用 maxY 降序）。
/// 同一行内的多段文本会各自成为一个 observation，因此排序只做粗粒度版面恢复，不做行的合并。
func readingOrder(_ first: RecognizedLine, _ second: RecognizedLine) -> Bool {
    let firstTop = first.box.maxY
    let secondTop = second.box.maxY
    if abs(firstTop - secondTop) > 0.01 {
        return firstTop > secondTop
    }
    return first.box.minX < second.box.minX
}

func recognizeText(in image: CGImage) throws -> [RecognizedLine] {
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    request.recognitionLanguages = requestedLanguages
    request.usesLanguageCorrection = true

    let handler = VNImageRequestHandler(cgImage: image, options: [:])
    do {
        try handler.perform([request])
    } catch {
        throw HelperError(
            code: .recognition,
            message: "Vision 识别失败：\(error.localizedDescription)"
        )
    }
    guard let observations = request.results else {
        return []
    }
    let lines = observations.compactMap { observation -> RecognizedLine? in
        guard let candidate = observation.topCandidates(1).first else {
            return nil
        }
        return RecognizedLine(
            text: candidate.string,
            confidence: candidate.confidence,
            box: observation.boundingBox
        )
    }
    return lines.sorted(by: readingOrder)
}

/// elapsedMs 覆盖「解码 + 识别」，不含进程启动与框架加载——后者由调用方的墙钟测量体现。
func runRecognition(path: String) throws -> (lines: [RecognizedLine], elapsedMs: Int) {
    let startedAt = DispatchTime.now().uptimeNanoseconds
    let image = try loadImage(at: path)
    let lines = try recognizeText(in: image)
    let elapsed = DispatchTime.now().uptimeNanoseconds - startedAt
    return (lines, Int(elapsed / 1_000_000))
}

/// 手写 JSON 转义而不是用 JSONEncoder：契约要求固定键序，而 JSONEncoder 不保证顺序。
/// 转义规则覆盖 JSON 规范要求的全部控制字符，避免图片里的异常字符产出非法 JSON。
func jsonLiteral(_ value: String) -> String {
    var result = "\""
    for scalar in value.unicodeScalars {
        switch scalar {
        case "\"":
            result += "\\\""
        case "\\":
            result += "\\\\"
        case "\n":
            result += "\\n"
        case "\r":
            result += "\\r"
        case "\t":
            result += "\\t"
        case "\u{08}":
            result += "\\b"
        case "\u{0C}":
            result += "\\f"
        default:
            if scalar.value < 0x20 {
                let hex = String(scalar.value, radix: 16)
                result += "\\u" + String(repeating: "0", count: 4 - hex.count) + hex
            } else {
                result.unicodeScalars.append(scalar)
            }
        }
    }
    result += "\""
    return result
}

/// 定点四位小数，不用 String(format:)：区域设置会影响小数点符号，JSON 必须是点。
func decimalLiteral(_ value: CGFloat) -> String {
    let scaled = Int((Double(value) * 10000).rounded())
    let sign = scaled < 0 ? "-" : ""
    let magnitude = abs(scaled)
    let fraction = magnitude % 10000
    let padded = String(repeating: "0", count: 4 - String(fraction).count) + String(fraction)
    return "\(sign)\(magnitude / 10000).\(padded)"
}

func successJSON(lines: [RecognizedLine], elapsedMs: Int) -> String {
    let fullText = lines.map(\.text).joined(separator: "\n")
    let encodedLines = lines.map { line -> String in
        let parts = [line.box.minX, line.box.minY, line.box.width, line.box.height]
            .map(decimalLiteral)
            .joined(separator: ",")
        let confidence = decimalLiteral(CGFloat(line.confidence))
        return "{\"text\":\(jsonLiteral(line.text)),\"confidence\":\(confidence),\"box\":[\(parts)]}"
    }
    let body = encodedLines.joined(separator: ",")
    return "{\"ok\":true,\"text\":\(jsonLiteral(fullText)),\"lines\":[\(body)],\"elapsedMs\":\(elapsedMs)}"
}

func failureJSON(error: String) -> String {
    return "{\"ok\":false,\"error\":\(jsonLiteral(error))}"
}

/// 用 FileHandle 直写：print 在管道下是块缓冲，配合 exit() 时会依赖 atexit 刷新，
/// 而 invoke 方是按行读取的，丢一行就是丢全部结果。
func emit(_ line: String) {
    guard let data = (line + "\n").data(using: .utf8) else {
        return
    }
    FileHandle.standardOutput.write(data)
}

func emitDiagnostic(_ line: String) {
    guard let data = (line + "\n").data(using: .utf8) else {
        return
    }
    FileHandle.standardError.write(data)
}

/// 诊断用：把本机 Vision 版本实际支持的语言打出来。
/// 契约固定请求 zh-Hans/zh-Hant/en-US，若某语言在别的机器上不被支持，perform() 会失败——
/// 有这张清单才能在那种机器上判断是「语言不支持」而不是「图有问题」。
func emitCapabilities() throws {
    let revisions = VNRecognizeTextRequest.supportedRevisions.sorted()
    let currentRevision = VNRecognizeTextRequest.currentRevision
    let request = VNRecognizeTextRequest()
    request.recognitionLevel = .accurate
    let languages: [String]
    do {
        languages = try request.supportedRecognitionLanguages()
    } catch {
        throw HelperError(
            code: .recognition,
            message: "无法查询支持的识别语言：\(error.localizedDescription)"
        )
    }
    let revisionList = revisions.map(String.init).joined(separator: ",")
    let languageList = languages.map(jsonLiteral).joined(separator: ",")
    emit(
        "{\"ok\":true,\"currentRevision\":\(currentRevision),"
            + "\"supportedRevisions\":[\(revisionList)],\"accurateLanguages\":[\(languageList)]}"
    )
}

let arguments = Array(CommandLine.arguments.dropFirst())
do {
    switch try parseCommand(arguments) {
    case .help:
        emit(usageText)
        exit(HelperExitCode.success.rawValue)
    case .capabilities:
        try emitCapabilities()
        exit(HelperExitCode.success.rawValue)
    case .recognize(let path):
        let result = try runRecognition(path: path)
        emit(successJSON(lines: result.lines, elapsedMs: result.elapsedMs))
        exit(HelperExitCode.success.rawValue)
    }
} catch let error as HelperError {
    emit(failureJSON(error: error.message))
    if error.code == .usage {
        emitDiagnostic(usageText)
    }
    exit(error.code.rawValue)
} catch {
    emit(failureJSON(error: "未预期的失败：\(error)"))
    exit(HelperExitCode.recognition.rawValue)
}
