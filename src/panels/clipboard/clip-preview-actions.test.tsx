/**
 * ClipPreview 操作按钮测试（缺陷2修复验证）。
 * 覆盖：五个操作按钮存在性、aria-label、回调接线、收藏 on 态、图片项无翻译按钮。
 */

import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { ClipPreview } from "./ClipPreview";

vi.mock("../../ipc/ipc-client", () => ({
  listClipItems: vi.fn(),
  deleteClipItem: vi.fn(),
  toggleFavoriteClip: vi.fn(),
  getClipImageOriginal: vi.fn(),
}));

vi.mock("../../panels/translate/browser-api", () => ({
  writeToClipboard: vi.fn().mockResolvedValue(undefined),
}));

import { writeToClipboard } from "../../panels/translate/browser-api";
import { getClipImageOriginal } from "../../ipc/ipc-client";

const mockWriteToClipboard = vi.mocked(writeToClipboard);
const mockGetClipImageOriginal = vi.mocked(getClipImageOriginal);

const TEXT_ITEM = {
  id: "item-1",
  content: "Hello World",
  kind: "text" as const,
  isFavorite: false,
  lastModifiedUtc: 1000,
};

const FAV_ITEM = {
  id: "item-2",
  content: "Favorite text",
  kind: "text" as const,
  isFavorite: true,
  lastModifiedUtc: 2000,
};

const IMAGE_ITEM = {
  id: "img-1",
  content: "[图片] 200x300",
  kind: "image" as const,
  isFavorite: false,
  lastModifiedUtc: 3000,
  imageId: "uuid-img-1",
  thumbnailDataUrl: "data:image/webp;base64,THUMB",
};

describe("ClipPreview 操作按钮", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mockGetClipImageOriginal.mockResolvedValue(null);
    mockWriteToClipboard.mockResolvedValue(undefined);
  });

  it("文本项渲染五个操作按钮（粘贴到前台/复制/一键翻译/收藏/删除）", () => {
    render(
      <ClipPreview
        item={TEXT_ITEM}
        onToggleFavorite={vi.fn()}
        onDelete={vi.fn()}
        onCopy={vi.fn()}
        onPasteToFront={vi.fn()}
        onTranslate={vi.fn()}
      />
    );

    expect(screen.getByRole("button", { name: "粘贴到前台" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "复制" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "一键翻译" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "收藏" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "删除" })).toBeInTheDocument();
  });

  it("点击复制按钮调用 onCopy", async () => {
    const onCopy = vi.fn();
    const user = userEvent.setup();

    render(
      <ClipPreview
        item={TEXT_ITEM}
        onToggleFavorite={vi.fn()}
        onDelete={vi.fn()}
        onCopy={onCopy}
        onPasteToFront={vi.fn()}
        onTranslate={vi.fn()}
      />
    );

    await user.click(screen.getByRole("button", { name: "复制" }));
    expect(onCopy).toHaveBeenCalledWith(TEXT_ITEM);
  });

  it("点击收藏按钮调用 onToggleFavorite", async () => {
    const onToggleFavorite = vi.fn();
    const user = userEvent.setup();

    render(
      <ClipPreview
        item={TEXT_ITEM}
        onToggleFavorite={onToggleFavorite}
        onDelete={vi.fn()}
        onCopy={vi.fn()}
        onPasteToFront={vi.fn()}
        onTranslate={vi.fn()}
      />
    );

    await user.click(screen.getByRole("button", { name: "收藏" }));
    expect(onToggleFavorite).toHaveBeenCalledWith(TEXT_ITEM);
  });

  it("点击删除按钮调用 onDelete", async () => {
    const onDelete = vi.fn();
    const user = userEvent.setup();

    render(
      <ClipPreview
        item={TEXT_ITEM}
        onToggleFavorite={vi.fn()}
        onDelete={onDelete}
        onCopy={vi.fn()}
        onPasteToFront={vi.fn()}
        onTranslate={vi.fn()}
      />
    );

    await user.click(screen.getByRole("button", { name: "删除" }));
    expect(onDelete).toHaveBeenCalledWith(TEXT_ITEM);
  });

  it("已收藏条目的收藏按钮带 .on 类", () => {
    render(
      <ClipPreview
        item={FAV_ITEM}
        onToggleFavorite={vi.fn()}
        onDelete={vi.fn()}
        onCopy={vi.fn()}
        onPasteToFront={vi.fn()}
        onTranslate={vi.fn()}
      />
    );

    const favBtn = screen.getByRole("button", { name: "收藏" });
    expect(favBtn.classList.contains("on")).toBe(true);
  });

  it("图片项有复制/收藏/删除/粘贴到前台按钮，无一键翻译", () => {
    render(
      <ClipPreview
        item={IMAGE_ITEM}
        onToggleFavorite={vi.fn()}
        onDelete={vi.fn()}
        onCopy={vi.fn()}
        onPasteToFront={vi.fn()}
        onTranslate={vi.fn()}
      />
    );

    expect(screen.getByRole("button", { name: "粘贴到前台" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "复制" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "一键翻译" })).not.toBeInTheDocument();
    expect(screen.getByRole("button", { name: "收藏" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "删除" })).toBeInTheDocument();
  });

  it("item 为 null 时不渲染操作按钮", () => {
    render(
      <ClipPreview
        item={null}
        onToggleFavorite={vi.fn()}
        onDelete={vi.fn()}
        onCopy={vi.fn()}
        onPasteToFront={vi.fn()}
        onTranslate={vi.fn()}
      />
    );

    expect(screen.queryByRole("button", { name: "复制" })).not.toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "删除" })).not.toBeInTheDocument();
  });
});
