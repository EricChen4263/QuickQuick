import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";

/**
 * 设置写失败的页面级错误通道，**按来源隔离**。
 *
 * 为什么要有它：各面板原先各自维护「保存失败」提示——外观/存储显示后端原文，
 * 隐私/热键/翻译源只显示通用文案，通用面板更是只 `console.warn`（表现为「点了没反应」）。
 * settings.json 损坏时后端会拒写并给出修复指引，这类错误必须让用户看到。
 *
 * 为什么要按来源隔离：通用面板三个开关共用一个错误槽时，A 开关失败后 B 开关成功
 * 会把 A 的失败原因一并清掉——用户看到错误闪一下就没了，却不知道到底哪项没保存。
 * 所以每条错误都带来源（`general.launchOnLogin`、`appearance.imageHeight`…），
 * **只有同来源的成功才清除该来源的错误**。
 *
 * Maccy 无对应逻辑（UserDefaults 写入没有失败路径）。业界做法：设置页顶部一条统一错误区。
 */

/** 错误来源标识，形如 `general.launchOnLogin`。 */
export type SaveErrorSource = string;

/** 错误条上的一条记录。 */
export interface SaveErrorEntry {
  source: SaveErrorSource;
  message: string;
}

/** `runSave` 的结果：成功带返回值，失败带后端中文原文。 */
export type SaveResult<T> = { ok: true; value: T } | { ok: false; error: string };

export interface SettingsSaveErrorChannel {
  /** 当前待展示的保存失败（按来源各一条，source 排序稳定）；空数组表示没有未处理的失败。 */
  saveErrors: SaveErrorEntry[];
  /** 上报一次保存失败（`error` 是 IPC reject 值，通常是后端返回的中文 String）。 */
  reportSaveError: (source: SaveErrorSource, error: unknown) => void;
  /** 清除**指定来源**的错误（同来源保存成功后调用）。 */
  clearSaveError: (source: SaveErrorSource) => void;
  /** 清除所有来源的错误（错误条上的「知道了」）。 */
  clearAllSaveErrors: () => void;
  /**
   * 包一层保存动作：成功即清除**该来源**的错误并返回 `{ ok: true, value }`；
   * 失败则上报后端原文并返回 `{ ok: false, error }`（供面板回滚输入/开关）。
   *
   * 不抛异常是有意的：面板的 catch 分支只需要「回滚 + 上报」，不需要判断错误类型。
   */
  runSave: <T>(source: SaveErrorSource, action: () => Promise<T>) => Promise<SaveResult<T>>;
}

/** 把 IPC reject 值转成可展示文案（后端返回的是中文字符串，Error 包装则取 message）。 */
export function saveErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string" && error.length > 0) return error;
  return String(error);
}

/**
 * 没有 Provider 时的兜底实现。
 *
 * 错误**不静默**：直接 `console.error` 留痕（面板单独渲染、或在 Provider 之外被使用时，
 * 至少日志里能查到原始错误），并且 `runSave` 仍把失败如实返回给调用方。
 */
const fallbackChannel: SettingsSaveErrorChannel = {
  saveErrors: [],
  reportSaveError: (source, error) => {
    console.error(`[settings] 保存失败（无页面级错误条，来源 ${source}）:`, saveErrorMessage(error));
  },
  clearSaveError: () => {},
  clearAllSaveErrors: () => {},
  runSave: async (source, action) => {
    try {
      return { ok: true, value: await action() };
    } catch (error) {
      console.error(
        `[settings] 保存失败（无页面级错误条，来源 ${source}）:`,
        saveErrorMessage(error),
      );
      return { ok: false, error: saveErrorMessage(error) };
    }
  },
};

const SettingsSaveErrorContext = createContext<SettingsSaveErrorChannel>(fallbackChannel);

/** 设置页根部的错误通道 Provider（错误条与各面板共用同一份状态）。 */
export function SettingsSaveErrorProvider({ children }: { children: ReactNode }) {
  const [errorsBySource, setErrorsBySource] = useState<Record<SaveErrorSource, string>>({});

  const reportSaveError = useCallback((source: SaveErrorSource, error: unknown): void => {
    setErrorsBySource((prev) => ({ ...prev, [source]: saveErrorMessage(error) }));
  }, []);

  const clearSaveError = useCallback((source: SaveErrorSource): void => {
    setErrorsBySource((prev) => {
      if (!(source in prev)) return prev;
      const next = { ...prev };
      delete next[source];
      return next;
    });
  }, []);

  const clearAllSaveErrors = useCallback((): void => {
    setErrorsBySource({});
  }, []);

  const runSave = useCallback(
    async <T,>(source: SaveErrorSource, action: () => Promise<T>): Promise<SaveResult<T>> => {
      try {
        const value = await action();
        setErrorsBySource((prev) => {
          if (!(source in prev)) return prev;
          const next = { ...prev };
          delete next[source];
          return next;
        });
        return { ok: true, value };
      } catch (error) {
        const message = saveErrorMessage(error);
        setErrorsBySource((prev) => ({ ...prev, [source]: message }));
        return { ok: false, error: message };
      }
    },
    [],
  );

  const saveErrors = useMemo<SaveErrorEntry[]>(
    () =>
      Object.keys(errorsBySource)
        .sort()
        .map((source) => ({ source, message: errorsBySource[source] })),
    [errorsBySource],
  );

  const channel = useMemo<SettingsSaveErrorChannel>(
    () => ({ saveErrors, reportSaveError, clearSaveError, clearAllSaveErrors, runSave }),
    [saveErrors, reportSaveError, clearSaveError, clearAllSaveErrors, runSave],
  );

  return (
    <SettingsSaveErrorContext.Provider value={channel}>{children}</SettingsSaveErrorContext.Provider>
  );
}

/** 取页面级保存错误通道。 */
export function useSettingsSaveError(): SettingsSaveErrorChannel {
  return useContext(SettingsSaveErrorContext);
}

/**
 * 页面级错误条：设置页顶部唯一的保存失败出口。
 *
 * `role="alert"` 让屏幕阅读器即时播报；每条文案都是后端原文（含「已拒绝写入……备份副本」
 * 这类修复指引），不做二次包装——包装会丢掉用户真正需要的路径信息。
 * 多来源同时失败时逐条展示：「哪一项没保存」本身就是信息，合并成一条会丢。
 */
export function SettingsSaveErrorBanner() {
  const { saveErrors, clearAllSaveErrors } = useSettingsSaveError();
  if (saveErrors.length === 0) return null;

  return (
    <div className="set-save-error" role="alert">
      <svg className="ico ico-16" viewBox="0 0 24 24" aria-hidden="true">
        <circle cx="12" cy="12" r="9" />
        <path d="M12 7.5v6" />
        <path d="M12 17h.01" />
      </svg>
      <div className="set-save-error-list">
        {saveErrors.map((entry) => (
          <div
            key={entry.source}
            className="set-save-error-text body-6"
            data-source={entry.source}
          >
            {entry.message}
          </div>
        ))}
      </div>
      <button
        type="button"
        className="set-save-error-dismiss button-sm"
        onClick={clearAllSaveErrors}
      >
        知道了
      </button>
    </div>
  );
}
