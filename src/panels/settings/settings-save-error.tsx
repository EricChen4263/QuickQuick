import { createContext, useCallback, useContext, useMemo, useState, type ReactNode } from "react";

/**
 * 设置写失败的页面级错误通道。
 *
 * 为什么要有它：各面板原先各自维护一条「保存失败」提示——外观/存储把后端原文显示出来了，
 * 隐私/热键/翻译源只显示通用文案，通用面板更是只 `console.warn`（表现为「点了没反应」）。
 * settings.json 损坏时后端会拒写并给出修复指引，这类错误必须让用户看到。
 *
 * Maccy 无对应逻辑（UserDefaults 写入没有失败路径，设置是逐键、也没有整文件）。
 * 这里采用业界常见做法：**设置页顶部一条统一错误条**，所有面板的保存失败都路由到这里，
 * 控件本身仍按原逻辑回滚（受控组件的 aria-checked / 输入框不撒谎）。
 */

/** `runSave` 的结果：成功带返回值，失败带后端中文原文。 */
export type SaveResult<T> = { ok: true; value: T } | { ok: false; error: string };

export interface SettingsSaveErrorChannel {
  /** 当前待展示的保存失败文案（后端原文）；null 表示没有未处理的失败。 */
  saveError: string | null;
  /** 上报一次保存失败（参数是 IPC reject 值，通常是后端返回的中文 String）。 */
  reportSaveError: (error: unknown) => void;
  /** 清空错误条（保存成功后调用）。 */
  clearSaveError: () => void;
  /**
   * 包一层保存动作：成功即清空错误条并返回 `{ ok: true, value }`；
   * 失败则上报后端原文并返回 `{ ok: false, error }`（供面板回滚输入/开关）。
   *
   * 不抛异常是有意的：面板的 catch 分支只需要「回滚 + 上报」，不需要自己判断错误类型。
   */
  runSave: <T>(action: () => Promise<T>) => Promise<SaveResult<T>>;
}

/** 把 IPC reject 值转成可展示文案（后端返回的是中文字符串，Error 包装则取 message）。 */
export function saveErrorMessage(error: unknown): string {
  if (error instanceof Error) return error.message;
  if (typeof error === "string" && error.length > 0) return error;
  return String(error);
}

const noop = (): void => {};

/**
 * 没有 Provider 时的兜底实现：面板可以单独渲染（例如各自的单测），
 * 错误只是不显示在页面级错误条上，不会抛错。
 */
const fallbackChannel: SettingsSaveErrorChannel = {
  saveError: null,
  reportSaveError: noop,
  clearSaveError: noop,
  runSave: async (action) => {
    try {
      return { ok: true, value: await action() };
    } catch (error) {
      console.warn("[settings] 保存失败（无页面级错误条）:", saveErrorMessage(error));
      return { ok: false, error: saveErrorMessage(error) };
    }
  },
};

const SettingsSaveErrorContext = createContext<SettingsSaveErrorChannel>(fallbackChannel);

/** 设置页根部的错误通道 Provider（错误条与各面板共用同一份状态）。 */
export function SettingsSaveErrorProvider({ children }: { children: ReactNode }) {
  const [saveError, setSaveError] = useState<string | null>(null);

  const reportSaveError = useCallback((error: unknown): void => {
    setSaveError(saveErrorMessage(error));
  }, []);

  const clearSaveError = useCallback((): void => {
    setSaveError(null);
  }, []);

  const runSave = useCallback(
    async <T,>(action: () => Promise<T>): Promise<SaveResult<T>> => {
      try {
        const value = await action();
        setSaveError(null);
        return { ok: true, value };
      } catch (error) {
        const message = saveErrorMessage(error);
        setSaveError(message);
        return { ok: false, error: message };
      }
    },
    [],
  );

  const channel = useMemo<SettingsSaveErrorChannel>(
    () => ({ saveError, reportSaveError, clearSaveError, runSave }),
    [saveError, reportSaveError, clearSaveError, runSave],
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
 * `role="alert"` 让屏幕阅读器即时播报；文案是后端原文（含「已拒绝写入……备份副本」这类修复指引），
 * 不做二次包装——包装会丢掉用户真正需要的路径信息。
 */
export function SettingsSaveErrorBanner() {
  const { saveError, clearSaveError } = useSettingsSaveError();
  if (saveError === null) return null;

  return (
    <div className="set-save-error" role="alert">
      <span className="set-save-error-text">{saveError}</span>
      <button type="button" className="set-save-error-dismiss" onClick={clearSaveError}>
        知道了
      </button>
    </div>
  );
}
