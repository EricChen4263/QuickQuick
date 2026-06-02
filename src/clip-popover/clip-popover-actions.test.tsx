import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, screen, fireEvent, waitFor } from "@testing-library/react";
import ClipPopoverApp from "./ClipPopoverApp";

vi.mock("../ipc/ipc-client", () => ({
  listClipItems: vi.fn(),
  pasteToFront: vi.fn(),
}));

vi.mock("../panels/translate/browser-api", () => ({
  writeToClipboard: vi.fn(),
}));

const mockHide = vi.fn();
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({ hide: mockHide }),
}));

import { listClipItems, pasteToFront } from "../ipc/ipc-client";
import { writeToClipboard } from "../panels/translate/browser-api";

const mockListClipItems = listClipItems as ReturnType<typeof vi.fn>;
const mockPasteToFront = pasteToFront as ReturnType<typeof vi.fn>;
const mockWriteToClipboard = writeToClipboard as ReturnType<typeof vi.fn>;

const ITEMS = [
  { id: "id1", content: "first item", kind: "text", isFavorite: false, lastModifiedUtc: Date.now() },
  { id: "id2", content: "second item", kind: "text", isFavorite: false, lastModifiedUtc: Date.now() },
  { id: "id3", content: "third item", kind: "text", isFavorite: false, lastModifiedUtc: Date.now() },
  {
    id: "id-img",
    content: "",
    kind: "image",
    isFavorite: false,
    lastModifiedUtc: Date.now(),
    thumbnailDataUrl: "data:image/png;base64,abc",
  },
];

beforeEach(() => {
  vi.clearAllMocks();
  mockListClipItems.mockResolvedValue(ITEMS);
  mockPasteToFront.mockResolvedValue({ outcome: "write_back_only" });
  mockWriteToClipboard.mockResolvedValue(undefined);
  mockHide.mockResolvedValue(undefined);
});

describe("ClipPopoverApp 键盘动作", () => {
  it("按 Enter 调 pasteToFront(selectedId) 并 hide", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("id1");
      expect(mockHide).toHaveBeenCalled();
    });
  });

  it("按 Alt+Enter 调 writeToClipboard(content) 并 hide", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "Enter", altKey: true });

    await waitFor(() => {
      expect(mockWriteToClipboard).toHaveBeenCalledWith("first item");
      expect(mockHide).toHaveBeenCalled();
    });
  });

  it("按 ArrowDown 选中第二项", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("second item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "ArrowDown" });

    await waitFor(() => {
      const rows = screen.getAllByRole("option");
      const secondRow = rows.find((r) => r.textContent?.includes("second item"));
      expect(secondRow).toHaveAttribute("aria-selected", "true");
    });
  });

  it("按 ArrowDown 再 Enter 粘贴第二项", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("second item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("id2");
      expect(mockHide).toHaveBeenCalled();
    });
  });

  it("图片条目按 Alt+Enter：writeToClipboard 和 hide 均不被调用", async () => {
    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByRole("option").length).toBeGreaterThan(0);
    });

    // 初始选中 id1，连按 3 次 ArrowDown 选中 id-img（第 4 项）
    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });
    fireEvent.keyDown(input, { key: "ArrowDown" });

    await waitFor(() => {
      const rows = screen.getAllByRole("option");
      // 第 4 项（index 3）是图片条目
      expect(rows[3]).toHaveAttribute("aria-selected", "true");
    });

    fireEvent.keyDown(input, { key: "Enter", altKey: true });

    await new Promise((r) => setTimeout(r, 50));
    expect(mockWriteToClipboard).not.toHaveBeenCalled();
    expect(mockHide).not.toHaveBeenCalled();
  });

  it("pasteToFront 失败时不调 hide，控制台有 error", async () => {
    mockPasteToFront.mockRejectedValue(new Error("paste failed"));
    const consoleError = vi.spyOn(console, "error").mockImplementation(() => {});

    render(<ClipPopoverApp />);

    await waitFor(() => {
      expect(screen.getAllByText("first item").length).toBeGreaterThan(0);
    });

    const input = screen.getByRole("searchbox");
    fireEvent.keyDown(input, { key: "Enter" });

    await waitFor(() => {
      expect(mockPasteToFront).toHaveBeenCalledWith("id1");
    });

    await new Promise((r) => setTimeout(r, 50));
    expect(mockHide).not.toHaveBeenCalled();
    expect(consoleError).toHaveBeenCalled();

    consoleError.mockRestore();
  });
});
