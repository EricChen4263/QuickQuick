import { describe, it, expect } from "vitest";
import {
  resolvePreviewState,
  PREVIEW_WAITING_TEXT,
  type PreviewState,
} from "./preview-state";
import type { ClipItem } from "../ipc/ipc-client";

function makeItem(): ClipItem {
  return {
    id: "id1",
    content: "first item",
    kind: "text",
    isFavorite: false,
    hasThumbnail: false,
    lastModifiedUtc: new Date("2026-06-02T10:00:00").getTime(),
  };
}

describe("resolvePreviewState 预览区三态判定", () => {
  it("有可显示条目时返回 item 态并带上该条目", () => {
    const item = makeItem();
    const state: PreviewState = resolvePreviewState(item, true);

    expect(state).toEqual({ kind: "item", item });
    // 必须原样传引用，不能复制一份（预览要用列表里最新的那个对象）
    expect(state.kind === "item" && state.item).toBe(item);
  });

  it("已选中但延迟未到点（还没有可显示条目）时返回 waiting 态", () => {
    expect(resolvePreviewState(null, true)).toEqual({ kind: "waiting" });
  });

  it("真的没有选中项时返回 empty 态", () => {
    expect(resolvePreviewState(null, false)).toEqual({ kind: "empty" });
  });

  it("有条目时优先显示条目：hasSelection=false 也不退化成空态", () => {
    const item = makeItem();
    expect(resolvePreviewState(item, false)).toEqual({ kind: "item", item });
  });

  it("占位文案是中性描述，不是「无选中项」也不是加载语义", () => {
    expect(PREVIEW_WAITING_TEXT).toBe("预览稍后显示");
    expect(PREVIEW_WAITING_TEXT).not.toContain("无选中项");
    expect(PREVIEW_WAITING_TEXT).not.toContain("加载");
  });
});
