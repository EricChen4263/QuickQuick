import { useEffect, useState, useCallback } from "react";
import { getStorageStats, cleanupHistory, getImageThreshold, setImageThreshold } from "../../ipc/ipc-client";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";

/** 存储上限：500 MB（字节） */
const MAX_BYTES = 500 * 1024 * 1024;

/** 默认单张图片阈值（MB） */
const DEFAULT_IMAGE_THRESHOLD_MB = 20;

/** 图片阈值预设档位（MB），全在后端合法区间 1..500 MiB 内 */
const IMAGE_THRESHOLD_OPTIONS = [5, 10, 20, 50, 100] as const;

/** 将字节数转为 MB 字符串，保留一位小数 */
function toMB(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1);
}

/** 存储子项面板：库体积进度条 + 上限/阈值展示 + 立即清理按钮 */
function StoragePanel() {
  const [liveCount, setLiveCount] = useState(0);
  const [fileSizeBytes, setFileSizeBytes] = useState(0);
  const [cleanupMsg, setCleanupMsg] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);
  const [imageThresholdMB, setImageThresholdMB] = useState(DEFAULT_IMAGE_THRESHOLD_MB);

  const fetchStats = useCallback(async (cancelled: { current: boolean }) => {
    try {
      const stats = await getStorageStats();
      if (cancelled.current) return;
      setLiveCount(stats.liveCount);
      setFileSizeBytes(stats.fileSizeBytes);
    } catch {
      if (cancelled.current) return;
      setOpError("存储统计加载失败，请稍后重试");
    }
  }, []);

  useEffect(() => {
    const cancelled = { current: false };
    fetchStats(cancelled);

    const loadThreshold = async () => {
      try {
        const bytes = await getImageThreshold();
        if (cancelled.current) return;
        setImageThresholdMB(Math.round(bytes / (1024 * 1024)));
      } catch {
        if (cancelled.current) return;
        console.error("图片阈值加载失败，使用默认值 20 MB");
      }
    };
    void loadThreshold();

    return () => {
      cancelled.current = true;
    };
  }, [fetchStats]);

  async function handleThresholdChange(mb: number) {
    try {
      await setImageThreshold(mb * 1024 * 1024);
      setImageThresholdMB(mb);
    } catch (err) {
      console.error("设置图片阈值失败:", err);
    }
  }

  async function handleCleanup() {
    setCleanupMsg(null);
    setOpError(null);
    try {
      const result = await cleanupHistory();
      const cancelled = { current: false };
      await fetchStats(cancelled);
      setCleanupMsg(`已清理 ${result.softDeleted + result.purged} 条`);
    } catch {
      setOpError("清理失败，请稍后重试");
    }
  }

  const meterWidth = Math.min((fileSizeBytes / MAX_BYTES) * 100, 100);

  return (
    <div>
      <PanelHeader title="存储" subtitle="加密库 quickquick.db · 分级清理，收藏永远豁免。" />
      <SettingGroup>
        <div className="set-row">
          <div className="grow">
            <div className="label">库体积</div>
            <div className="meter" aria-hidden="true">
              <i style={{ width: `${meterWidth}%` }} />
            </div>
            <div className="meter-legend">
              <span>{toMB(fileSizeBytes)} MB 已用</span>
              <span>上限 500 MB</span>
            </div>
          </div>
        </div>
        <div className="set-row">
          <div className="grow">
            <div className="label">条目数</div>
            <div className="desc">{liveCount} 条活跃记录（不含软删）</div>
          </div>
        </div>
        <div className="set-row">
          <div className="grow">
            <div className="label">总量上限</div>
            <div className="desc">超出按分级清理：先删大图原图保缩略图，再整条清最旧非收藏</div>
          </div>
          <span className="kbd-combo num">
            <kbd>500 MB</kbd>
          </span>
        </div>
        <div className="set-row">
          <div className="grow">
            <label htmlFor="image-threshold-select" className="label">单张图片阈值</label>
            <div className="desc">超过此大小的图片只保留缩略图，原图标记为过大未存</div>
          </div>
          <select
            id="image-threshold-select"
            aria-label="单张图片阈值"
            className="sel"
            value={String(imageThresholdMB)}
            onChange={(e) => { void handleThresholdChange(Number(e.target.value)); }}
          >
            {IMAGE_THRESHOLD_OPTIONS.map((mb) => (
              <option key={mb} value={String(mb)}>
                {mb} MB
              </option>
            ))}
          </select>
        </div>
        <div className="set-row">
          <div className="grow">
            <div className="label">立即清理非收藏历史</div>
            <div className="desc">物理清除已软删条目（墓碑 GC）</div>
          </div>
          <button className="btn" type="button" onClick={() => { void handleCleanup(); }}>
            清理…
          </button>
        </div>
      </SettingGroup>

      {cleanupMsg !== null && (
        <div style={{ marginTop: 8, fontSize: 12, color: "var(--muted)" }}>
          {cleanupMsg}
        </div>
      )}
      {opError !== null && (
        <div role="alert" style={{ color: "var(--danger)", marginTop: 8, fontSize: 12 }}>
          {opError}
        </div>
      )}
    </div>
  );
}

export default StoragePanel;
