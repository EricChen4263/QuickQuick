/**
 * 首次运行辅助功能权限引导卡片。
 * 用 localStorage key qq-onboard-dismissed 控制显隐，由父组件 ClipboardPage 管理。
 * onOpenSystemSettings 里程碑2 暂为 noop，里程碑3 接 IPC 跳系统设置。
 */

interface OnboardCardProps {
  onDismiss: () => void;
  onOpenSystemSettings: () => void;
}

/** 辅助功能引导卡片：图标 + 标题 + 说明 + 操作按钮 + 关闭 × */
export function OnboardCard({ onDismiss, onOpenSystemSettings }: OnboardCardProps) {
  return (
    <div className="onboard">
      <svg
        className="lead"
        viewBox="0 0 24 24"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.7"
        strokeLinecap="round"
        strokeLinejoin="round"
        aria-hidden="true"
      >
        <circle cx="12" cy="12" r="10" />
        <path d="M12 16v-4M12 8h.01" />
      </svg>
      <div className="grow">
        <h4>开启「辅助功能」以启用自动粘贴</h4>
        <p>
          授权后可模拟 ⌘V 自动回写选中条目；未授权时降级为仅复制到剪贴板，功能不中断。
        </p>
        <div className="row">
          <button
            className="btn btn-primary"
            type="button"
            onClick={onOpenSystemSettings}
          >
            前往系统设置
          </button>
          <button
            className="btn btn-ghost"
            type="button"
            onClick={onDismiss}
          >
            稍后
          </button>
        </div>
      </div>
      <button
        className="x"
        type="button"
        aria-label="关闭"
        onClick={onDismiss}
      >
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.9"
          strokeLinecap="round"
          aria-hidden="true"
        >
          <path d="M18 6 6 18M6 6l12 12" />
        </svg>
      </button>
    </div>
  );
}
