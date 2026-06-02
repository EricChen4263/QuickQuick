import { describe, it, expect } from "vitest";
import type { ClipItem } from "../ipc/ipc-client";
import { pickLatestText } from "./source-text";

function makeTextItem(content: string): ClipItem {
  return {
    id: "item-1",
    content,
    kind: "text",
    isFavorite: false,
    lastModifiedUtc: 1000,
  };
}

function makeImageItem(content = ""): ClipItem {
  return {
    id: "img-1",
    content,
    kind: "image",
    isFavorite: false,
    lastModifiedUtc: 2000,
    thumbnailDataUrl: "data:image/webp;base64,abc",
    imageId: "img-id-1",
  };
}

function makeRichtextItem(content: string): ClipItem {
  return {
    id: "rt-1",
    content,
    kind: "richtext",
    isFavorite: false,
    lastModifiedUtc: 3000,
  };
}

describe("pickLatestText", () => {
  it("正常文本条目返回 [0] 的 content", () => {
    const items: ClipItem[] = [makeTextItem("hello world"), makeTextItem("older")];
    expect(pickLatestText(items)).toBe("hello world");
  });

  it("空数组返回 null", () => {
    expect(pickLatestText([])).toBeNull();
  });

  it("[0] 是空串返回 null", () => {
    expect(pickLatestText([makeTextItem("")])).toBeNull();
  });

  it("[0] 是纯空白返回 null", () => {
    expect(pickLatestText([makeTextItem("   \n  ")])).toBeNull();
  });

  it("[0] 是图片项（content 空）返回 null", () => {
    expect(pickLatestText([makeImageItem()])).toBeNull();
  });

  it("L-3：图片项即使 content 非空也返回 null（不可译）", () => {
    expect(pickLatestText([makeImageItem("some text")])).toBeNull();
  });

  it("L-3：富文本项返回其 content（可译）", () => {
    expect(pickLatestText([makeRichtextItem("rich content")])).toBe("rich content");
  });
});
