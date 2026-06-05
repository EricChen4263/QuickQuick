import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { restartApp } from "../ipc/ipc-client";
import "./UpdateBanner.css";

/** `update://ready` 事件 payload，camelCase 对齐 Rust `UpdateReadyPayload`。 */
interface UpdateReadyPayload {
  version: string;
}

/** 事件名与后端 `UPDATE_READY_EVENT` 常量一致。 */
const UPDATE_READY_EVENT = "update://ready";

/**
 * 全局更新就绪提示条：监听 `update://ready`，渲染非打扰提示，
 * 提供「重启更新」（调 restartApp）与「稍后」（隐藏）两个操作。
 *
 * 自包含监听——更新提示是跨页全局关注点，挂在 AppShell 顶层一处即可，
 * 无需父组件传递事件，故组件内部自行 listen 并管理就绪版本号状态。
 */
function UpdateBanner() {
  const [readyVersion, setReadyVersion] = useState<string | null>(null);

  // 沿用 App.tsx 的 listen 注册惯例：cancelled flag 防卸载后 resolve 泄漏监听器
  useEffect(() => {
    let cancelled = false;
    let unlisten: (() => void) | undefined;

    listen<UpdateReadyPayload>(UPDATE_READY_EVENT, (event) => {
      setReadyVersion((_prev) => event.payload.version);
    })
      .then((fn) => {
        if (cancelled) {
          fn();
        } else {
          unlisten = fn;
        }
      })
      .catch((err: unknown) => {
        console.error("[QuickQuick] update://ready 监听注册失败:", err);
      });

    return () => {
      cancelled = true;
      unlisten?.();
    };
  }, []);

  function handleRestart() {
    restartApp().catch((err: unknown) => {
      console.error("[QuickQuick] 重启更新失败:", err);
    });
  }

  if (readyVersion === null) {
    return null;
  }

  return (
    <div className="update-banner" role="status">
      <span className="update-banner-text">新版本 {readyVersion} 已就绪</span>
      <div className="update-banner-actions">
        <button type="button" className="btn btn-primary" onClick={handleRestart}>
          重启更新
        </button>
        <button
          type="button"
          className="btn btn-ghost"
          onClick={() => setReadyVersion((_prev) => null)}
        >
          稍后
        </button>
      </div>
    </div>
  );
}

export default UpdateBanner;
