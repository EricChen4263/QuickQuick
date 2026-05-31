import { describe, it, expect, vi, beforeEach } from "vitest";
import type { MockedFunction } from "vitest";

// Mock 必须在 import 实现之前声明（vitest hoisting）
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

import { invoke } from "@tauri-apps/api/core";
import {
  listClipItems,
  deleteClipItem,
  toggleFavoriteClip,
  translateText,
  listTranslateHistory,
  getHotkeys,
  setHotkey,
  getExcludeList,
  setExcludeList,
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
} from "./ipc-client";
import type {
  ClipItem,
  TranslateResult,
  TranslateHistoryItem,
  Provider,
  Hotkeys,
} from "./ipc-client";

const mockInvoke = invoke as MockedFunction<typeof invoke>;

beforeEach(() => {
  mockInvoke.mockReset();
});

describe("listClipItems", () => {
  it("使用正确命令名调用 invoke 并透传返回的数组", async () => {
    const items: ClipItem[] = [
      {
        id: "abc",
        content: "hello",
        kind: "text",
        isFavorite: false,
        lastModifiedUtc: 1000,
      },
    ];
    mockInvoke.mockResolvedValueOnce(items);

    const result = await listClipItems();

    expect(mockInvoke).toHaveBeenCalledWith("list_clip_items");
    expect(result).toEqual(items);
  });

  it("invoke reject 字符串时重抛为 Error 且含原始消息", async () => {
    mockInvoke.mockRejectedValueOnce("数据库连接失败");

    const err = await listClipItems().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("数据库连接失败");
  });
});

describe("deleteClipItem", () => {
  it("传入正确命令名与 id 参数", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await deleteClipItem("item-123");

    expect(mockInvoke).toHaveBeenCalledWith("delete_clip_item", {
      id: "item-123",
    });
  });
});

describe("toggleFavoriteClip", () => {
  it("传入正确命令名、id、favorite 参数（true）", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await toggleFavoriteClip("item-456", true);

    expect(mockInvoke).toHaveBeenCalledWith("toggle_favorite_clip", {
      id: "item-456",
      favorite: true,
    });
  });

  it("传入 favorite=false 时参数正确", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await toggleFavoriteClip("item-789", false);

    expect(mockInvoke).toHaveBeenCalledWith("toggle_favorite_clip", {
      id: "item-789",
      favorite: false,
    });
  });

  it("invoke reject 字符串时重抛为 Error", async () => {
    mockInvoke.mockRejectedValueOnce("收藏操作失败");

    const err = await toggleFavoriteClip("bad-id", true).catch((e: unknown) => e);
    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("收藏操作失败");
  });
});

describe("translateText", () => {
  it("不带 target 时使用正确命令名与文本参数", async () => {
    const result: TranslateResult = {
      translated: "Hello",
      sourceLang: "zh",
      targetLang: "en",
    };
    mockInvoke.mockResolvedValueOnce(result);

    const ret = await translateText("你好");

    expect(mockInvoke).toHaveBeenCalledWith("translate_text", {
      text: "你好",
      target: undefined,
    });
    expect(ret).toEqual(result);
  });

  it("带 target 时正确传递 target 参数", async () => {
    const result: TranslateResult = {
      translated: "Bonjour",
      sourceLang: "zh",
      targetLang: "fr",
    };
    mockInvoke.mockResolvedValueOnce(result);

    await translateText("你好", "fr");

    expect(mockInvoke).toHaveBeenCalledWith("translate_text", {
      text: "你好",
      target: "fr",
    });
  });

  it("invoke reject 字符串时重抛为含原始消息的 Error", async () => {
    mockInvoke.mockRejectedValueOnce("翻译服务不可用");

    const err = await translateText("hello").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("翻译服务不可用");
  });
});

describe("listTranslateHistory", () => {
  it("使用正确命令名调用 invoke 并透传历史数组", async () => {
    const history: TranslateHistoryItem[] = [
      {
        id: "h1",
        sourceText: "hello",
        translatedText: "你好",
        sourceLang: "en",
        targetLang: "zh",
        providerId: "mymemory",
        createdUtc: 2000,
      },
    ];
    mockInvoke.mockResolvedValueOnce(history);

    const result = await listTranslateHistory();

    expect(mockInvoke).toHaveBeenCalledWith("list_translate_history");
    expect(result).toEqual(history);
  });
});

describe("getHotkeys", () => {
  it("使用正确命令名调用 invoke 并透传热键配置", async () => {
    const hotkeys: Hotkeys = { history: "CmdOrCtrl+Shift+H", translate: "CmdOrCtrl+Shift+T" };
    mockInvoke.mockResolvedValueOnce(hotkeys);

    const result = await getHotkeys();

    expect(mockInvoke).toHaveBeenCalledWith("get_hotkeys");
    expect(result).toEqual(hotkeys);
  });
});

describe("setHotkey", () => {
  it("传入正确命令名、action、accelerator 参数", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await setHotkey("history", "CmdOrCtrl+H");

    expect(mockInvoke).toHaveBeenCalledWith("set_hotkey", {
      action: "history",
      accelerator: "CmdOrCtrl+H",
    });
  });

  it("translate action 参数正确传递", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await setHotkey("translate", "CmdOrCtrl+T");

    expect(mockInvoke).toHaveBeenCalledWith("set_hotkey", {
      action: "translate",
      accelerator: "CmdOrCtrl+T",
    });
  });

  it("invoke reject 字符串时重抛为含原始消息的 Error", async () => {
    mockInvoke.mockRejectedValueOnce("热键已被占用");

    const err = await setHotkey("history", "CmdOrCtrl+H").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("热键已被占用");
  });
});

describe("getExcludeList", () => {
  it("使用正确命令名调用 invoke 并透传字符串数组", async () => {
    const list = ["com.example.app", "com.another.app"];
    mockInvoke.mockResolvedValueOnce(list);

    const result = await getExcludeList();

    expect(mockInvoke).toHaveBeenCalledWith("get_exclude_list");
    expect(result).toEqual(list);
  });
});

describe("setExcludeList", () => {
  it("传入正确命令名与 list 参数", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await setExcludeList(["com.example.app"]);

    expect(mockInvoke).toHaveBeenCalledWith("set_exclude_list", {
      list: ["com.example.app"],
    });
  });
});

describe("getTranslateProviders", () => {
  it("使用正确命令名调用 invoke 并透传 Provider 数组", async () => {
    const providers: Provider[] = [
      { id: "mymemory", name: "MyMemory", needsKey: false },
    ];
    mockInvoke.mockResolvedValueOnce(providers);

    const result = await getTranslateProviders();

    expect(mockInvoke).toHaveBeenCalledWith("get_translate_providers");
    expect(result).toEqual(providers);
  });
});

describe("getSelectedProvider", () => {
  it("使用正确命令名调用 invoke 并透传 provider id 字符串", async () => {
    mockInvoke.mockResolvedValueOnce("mymemory");

    const result = await getSelectedProvider();

    expect(mockInvoke).toHaveBeenCalledWith("get_selected_provider");
    expect(result).toBe("mymemory");
  });
});

describe("setSelectedProvider", () => {
  it("传入正确命令名与 id 参数", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await setSelectedProvider("mymemory");

    expect(mockInvoke).toHaveBeenCalledWith("set_selected_provider", {
      id: "mymemory",
    });
  });

  it("invoke reject 字符串时重抛为含原始消息的 Error", async () => {
    mockInvoke.mockRejectedValueOnce("非法 provider id");

    const err = await setSelectedProvider("invalid").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("非法 provider id");
  });
});
