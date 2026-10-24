import { describe, it, expect } from "vitest";
import {
  RENDER_BLOCK_SIZE,
  NEAR_BOTTOM_THRESHOLD_PX,
  initialRenderCount,
  growRenderCount,
  nextRenderCount,
  shouldResetRenderCount,
  isNearBottom,
} from "./render-window";

describe("initialRenderCount", () => {
  it("总数远超一块时只渲染一块", () => {
    // Arrange / Act
    const count = initialRenderCount(250);

    // Assert
    expect(count).toBe(100);
  });

  it("总数不足一块时全渲染", () => {
    expect(initialRenderCount(30)).toBe(30);
  });

  it("空列表窗口为 0", () => {
    expect(initialRenderCount(0)).toBe(0);
  });
});

describe("growRenderCount", () => {
  it("每次追加一块", () => {
    expect(growRenderCount(100, 250)).toBe(200);
  });

  it("追加不超过总数", () => {
    // 200 + 100 会越过总数 250，应收敛到 250
    expect(growRenderCount(200, 250)).toBe(250);
  });

  it("已经渲染全部时保持不变", () => {
    expect(growRenderCount(250, 250)).toBe(250);
  });

  it("空列表不增长", () => {
    expect(growRenderCount(100, 0)).toBe(0);
  });
});

describe("nextRenderCount", () => {
  it("选中项在窗口内时不扩张", () => {
    // Arrange：选中第 50 条，窗口 100 已覆盖
    const count = nextRenderCount(100, 50, 250);

    // Assert
    expect(count).toBe(100);
  });

  it("选中项刚好等于窗口大小时扩到下一块边界", () => {
    // Arrange：窗口 100 覆盖索引 0..99，索引 100 是第一条未渲染的记录
    const count = nextRenderCount(100, 100, 250);

    // Assert：按块对齐扩张，而不是只加一条
    expect(count).toBe(200);
  });

  it("选中项是最后一条时窗口扩到总数", () => {
    // Arrange：⌘↓ 跳末条 → 索引 249
    const count = nextRenderCount(100, 249, 250);

    // Assert
    expect(count).toBe(250);
  });

  it("同一块内的后续移动不改变窗口", () => {
    // Arrange：扩张到 200 后，索引 150 与 199 都已在窗口内
    const first = nextRenderCount(100, 120, 250);
    const second = nextRenderCount(first, 150, 250);

    // Assert
    expect(first).toBe(200);
    expect(second).toBe(200);
  });

  it("总数小于窗口时不扩张", () => {
    // Arrange：总数 30，选中最后一条
    const count = nextRenderCount(100, 29, 30);

    // Assert：窗口收敛到总数，不增不减
    expect(count).toBe(30);
  });

  it("无选中项（索引 -1）时窗口不变", () => {
    expect(nextRenderCount(100, -1, 250)).toBe(100);
  });

  it("窗口大于总数时收敛到总数，不越界", () => {
    expect(nextRenderCount(200, 5, 120)).toBe(120);
  });

  it("块大小是窗口增长的唯一粒度", () => {
    // 选中第 101 条（索引 100）→ 需要 101 条，向上取整到 2 块
    expect(nextRenderCount(RENDER_BLOCK_SIZE, 100, 1000)).toBe(RENDER_BLOCK_SIZE * 2);
  });
});

describe("shouldResetRenderCount", () => {
  it("搜索词变化时要求重置", () => {
    expect(shouldResetRenderCount("", "hello")).toBe(true);
  });

  it("搜索词清空时同样要求重置", () => {
    expect(shouldResetRenderCount("hello", "")).toBe(true);
  });

  it("搜索词未变时不重置", () => {
    expect(shouldResetRenderCount("hello", "hello")).toBe(false);
  });
});

describe("isNearBottom", () => {
  it("滚到底部时进入追加区", () => {
    // 内容 2000，视口 400，滚到 1600 即触底
    expect(isNearBottom(1600, 2000, 400)).toBe(true);
  });

  it("距离底部恰好等于阈值时进入追加区", () => {
    // 该 scrollTop 下 scrollHeight - scrollTop - clientHeight 恰好等于阈值
    const scrollTop = 2000 - 400 - NEAR_BOTTOM_THRESHOLD_PX;
    expect(isNearBottom(scrollTop, 2000, 400)).toBe(true);
  });

  it("超出阈值 1px 时不追加", () => {
    const scrollTop = 2000 - 400 - NEAR_BOTTOM_THRESHOLD_PX - 1;
    expect(isNearBottom(scrollTop, 2000, 400)).toBe(false);
  });

  it("停在顶部时不追加", () => {
    expect(isNearBottom(0, 2000, 400)).toBe(false);
  });

  it("内容未溢出视口时视为已到底部", () => {
    expect(isNearBottom(0, 300, 400)).toBe(true);
  });
});
