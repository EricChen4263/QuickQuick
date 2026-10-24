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
  getExcludeMode,
  setExcludeMode,
  getIgnoreRegexes,
  setIgnoreRegexes,
  getTranslateProviders,
  getSelectedProvider,
  setSelectedProvider,
  exportBackup,
  importBackup,
  revealBackupInFinder,
  getStorageStats,
} from "./ipc-client";
import type {
  ClipItem,
  TranslateResult,
  TranslateHistoryItem,
  Provider,
  Hotkeys,
  ExportBackupResult,
  ImportBackupResult,
  StorageStats,
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
        hasThumbnail: false,
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
      kind: "plain",
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
      kind: "plain",
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
    const hotkeys: Hotkeys = {
      history: "CmdOrCtrl+Shift+H",
      translate: "CmdOrCtrl+Shift+T",
      main: "CmdOrCtrl+Shift+M",
    };
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

  it("main action 参数正确传递", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await setHotkey("main", "CmdOrCtrl+M");

    expect(mockInvoke).toHaveBeenCalledWith("set_hotkey", {
      action: "main",
      accelerator: "CmdOrCtrl+M",
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

describe("getExcludeMode", () => {
  it("使用正确命令名调用 invoke 并透传模式字符串", async () => {
    mockInvoke.mockResolvedValueOnce("whitelist");

    const result = await getExcludeMode();

    expect(mockInvoke).toHaveBeenCalledWith("get_exclude_mode");
    expect(result).toBe("whitelist");
  });

  it("invoke reject 字符串时重抛为含原始消息的 Error", async () => {
    mockInvoke.mockRejectedValueOnce("读取模式失败");

    await expect(getExcludeMode()).rejects.toThrow("读取模式失败");
  });
});

describe("setExcludeMode", () => {
  it("传入正确命令名与 mode 参数", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await setExcludeMode("whitelist");

    expect(mockInvoke).toHaveBeenCalledWith("set_exclude_mode", {
      mode: "whitelist",
    });
  });

  it("invoke reject 字符串时重抛为含原始消息的 Error", async () => {
    mockInvoke.mockRejectedValueOnce("写入模式失败");

    await expect(setExcludeMode("blacklist")).rejects.toThrow("写入模式失败");
  });
});

describe("getIgnoreRegexes", () => {
  it("使用正确命令名调用 invoke 并透传正则原文数组", async () => {
    const patterns = ["^\\d{16}$", "([unclosed"];
    mockInvoke.mockResolvedValueOnce(patterns);

    const result = await getIgnoreRegexes();

    expect(mockInvoke).toHaveBeenCalledWith("get_ignore_regexes");
    expect(result).toEqual(patterns);
  });

  it("invoke reject 字符串时重抛为含原始消息的 Error", async () => {
    mockInvoke.mockRejectedValueOnce("读取正则失败");

    await expect(getIgnoreRegexes()).rejects.toThrow("读取正则失败");
  });
});

describe("setIgnoreRegexes", () => {
  it("传入正确命令名与 patterns 参数", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await setIgnoreRegexes(["^secret-"]);

    expect(mockInvoke).toHaveBeenCalledWith("set_ignore_regexes", {
      patterns: ["^secret-"],
    });
  });

  it("invoke reject 字符串时重抛为含原始消息的 Error", async () => {
    mockInvoke.mockRejectedValueOnce("写入正则失败");

    await expect(setIgnoreRegexes([])).rejects.toThrow("写入正则失败");
  });
});

describe("getTranslateProviders", () => {
  it("使用正确命令名调用 invoke 并透传 Provider 数组", async () => {
    const providers: Provider[] = [
      { id: "mymemory", name: "MyMemory", needsKey: false, needsConfig: false, isUnofficial: true },
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

describe("exportBackup", () => {
  it("使用正确命令名并传 passphrase 参数，透传返回的导出结果", async () => {
    const result: ExportBackupResult = {
      path: "/Users/tester/Documents/QuickQuick-Backup-20231114-221320.qqbp",
      itemCount: 3,
      skippedCount: 1,
      usedFallbackDir: false,
    };
    mockInvoke.mockResolvedValueOnce(result);

    const ret = await exportBackup("backup-passphrase");

    expect(mockInvoke).toHaveBeenCalledWith("export_backup", {
      passphrase: "backup-passphrase",
    });
    expect(ret).toEqual(result);
  });

  it("invoke reject 字符串时重抛为含原始消息的 Error", async () => {
    mockInvoke.mockRejectedValueOnce("请输入备份口令");

    const err = await exportBackup("").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("请输入备份口令");
  });
});

describe("importBackup", () => {
  it("使用正确命令名并传 blobBase64 与 passphrase 参数", async () => {
    const result: ImportBackupResult = { inserted: 2, skipped: 1 };
    mockInvoke.mockResolvedValueOnce(result);

    const ret = await importBackup("QUJD", "backup-passphrase");

    // blobBase64 对应 Rust 侧 blob_base64（Tauri 命令参数按 camelCase 映射）
    expect(mockInvoke).toHaveBeenCalledWith("import_backup", {
      blobBase64: "QUJD",
      passphrase: "backup-passphrase",
    });
    expect(ret).toEqual(result);
  });

  it("invoke reject 字符串时重抛为含原始消息的 Error（口令错误文案透传）", async () => {
    mockInvoke.mockRejectedValueOnce("备份口令错误，或文件已损坏");

    const err = await importBackup("QUJD", "wrong").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toBe("备份口令错误，或文件已损坏");
  });
});

describe("revealBackupInFinder", () => {
  it("使用正确命令名并传 path 参数", async () => {
    mockInvoke.mockResolvedValueOnce(undefined);

    await revealBackupInFinder("/Users/tester/Documents/QuickQuick-Backup-20231114-221320.qqbp");

    expect(mockInvoke).toHaveBeenCalledWith("reveal_backup_in_finder", {
      path: "/Users/tester/Documents/QuickQuick-Backup-20231114-221320.qqbp",
    });
  });

  it("invoke reject 字符串时重抛为含原始消息的 Error", async () => {
    mockInvoke.mockRejectedValueOnce("只能在文件管理器中显示 .qqbp 备份文件");

    const err = await revealBackupInFinder("/tmp/x.txt").catch((e: unknown) => e);
    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("只能在文件管理器中显示 .qqbp 备份文件");
  });
});

describe("getStorageStats", () => {
  it("使用正确命令名调用 invoke，并透传图片占用与库文件体积两个字段", async () => {
    const stats: StorageStats = {
      liveCount: 12,
      imageBytes: 3 * 1024 * 1024,
      fileSizeBytes: 40 * 1024 * 1024,
    };
    mockInvoke.mockResolvedValueOnce(stats);

    const result = await getStorageStats();

    expect(mockInvoke).toHaveBeenCalledWith("get_storage_stats");
    expect(result).toEqual(stats);
    // 两个口径是不同的量，必须各自原样透传，不能互相顶替
    expect(result.imageBytes).toBe(3 * 1024 * 1024);
    expect(result.fileSizeBytes).toBe(40 * 1024 * 1024);
  });

  it("invoke reject 字符串时重抛为 Error 且含原始消息", async () => {
    mockInvoke.mockRejectedValueOnce("存储统计不可用");

    const err = await getStorageStats().catch((e: unknown) => e);
    expect(err).toBeInstanceOf(Error);
    expect((err as Error).message).toContain("存储统计不可用");
  });
});
