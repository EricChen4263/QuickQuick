import { describe, it, expect } from "vitest";
import {
  ZOOM_MAX,
  ZOOM_MIN,
  ZOOM_STEP,
  clampZoom,
  effectiveScale,
  fitScale,
  formatZoomPercent,
  stepZoom,
  viewerKeyAction,
} from "./image-viewer-zoom";

describe("stepZoom 逐级缩放", () => {
  it("放大一步乘以步长，缩小一步除以步长", () => {
    expect(stepZoom(1, "in")).toBeCloseTo(ZOOM_STEP, 10);
    expect(stepZoom(1, "out")).toBeCloseTo(1 / ZOOM_STEP, 10);
  });

  it("放大到上限后不再增长", () => {
    expect(stepZoom(ZOOM_MAX, "in")).toBe(ZOOM_MAX);
  });

  it("缩小到下限后不再减小", () => {
    expect(stepZoom(ZOOM_MIN, "out")).toBe(ZOOM_MIN);
  });
});

describe("clampZoom 合法范围", () => {
  it("超出范围的比例被夹到边界，范围内的比例原样返回", () => {
    expect(clampZoom(100)).toBe(ZOOM_MAX);
    expect(clampZoom(0.001)).toBe(ZOOM_MIN);
    expect(clampZoom(2)).toBe(2);
  });
});

describe("fitScale 适应窗口的实际比例", () => {
  it("渲染宽度与原始宽度的比值即为适应窗口时的比例", () => {
    expect(fitScale(400, 1600)).toBe(0.25);
  });

  it("渲染宽度无效（为 0，布局未发生）时按原比例 1 处理", () => {
    expect(fitScale(0, 1600)).toBe(1);
  });

  it("原始宽度无效时按原比例 1 处理，避免除零", () => {
    expect(fitScale(400, 0)).toBe(1);
  });
});

describe("effectiveScale 当前有效比例", () => {
  it("显式比例直接使用，不受渲染宽度影响", () => {
    expect(effectiveScale(2, 400, 1600)).toBe(2);
  });

  it("适应窗口（null）时按渲染结果反推", () => {
    expect(effectiveScale(null, 400, 1600)).toBe(0.25);
  });
});

describe("formatZoomPercent 比例文案", () => {
  it("按百分比取整显示", () => {
    expect(formatZoomPercent(1.25)).toBe("125%");
    expect(formatZoomPercent(1)).toBe("100%");
  });
});

describe("viewerKeyAction 按键映射", () => {
  it("Esc 关闭；+ 与 = 放大；- 与 _ 缩小；0 适应；1 原比例", () => {
    expect(viewerKeyAction("Escape")).toBe("close");
    expect(viewerKeyAction("+")).toBe("zoom-in");
    expect(viewerKeyAction("=")).toBe("zoom-in");
    expect(viewerKeyAction("-")).toBe("zoom-out");
    expect(viewerKeyAction("_")).toBe("zoom-out");
    expect(viewerKeyAction("0")).toBe("fit");
    expect(viewerKeyAction("1")).toBe("actual");
  });

  it("无关按键返回 null，由调用方放行", () => {
    expect(viewerKeyAction("ArrowDown")).toBeNull();
    expect(viewerKeyAction("a")).toBeNull();
    expect(viewerKeyAction("2")).toBeNull();
  });
});
