import { useEffect, useState, useCallback, useRef } from "react";
import type { ChangeEvent } from "react";
import {
  getStorageStats,
  cleanupHistory,
  getImageThreshold,
  setImageThreshold,
  exportBackup,
  importBackup,
  revealBackupInFinder,
} from "../../ipc/ipc-client";
import { Select } from "../../components/Select";
import type { SelectOption } from "../../components/Select";
import { arrayBufferToBase64 } from "./backup-file";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";

/** 存储上限：500 MB（字节） */
const MAX_BYTES = 500 * 1024 * 1024;

/** 默认单张图片阈值（MB） */
const DEFAULT_IMAGE_THRESHOLD_MB = 20;

/** 图片阈值预设档位（MB），全在后端合法区间 1..500 MiB 内 */
const IMAGE_THRESHOLD_OPTIONS = [5, 10, 20, 50, 100] as const;

/** 图片阈值下拉选项（档位 → Select 选项） */
const IMAGE_THRESHOLD_SELECT_OPTIONS: SelectOption[] = IMAGE_THRESHOLD_OPTIONS.map((mb) => ({
  value: String(mb),
  label: `${mb} MB`,
}));

/** 将字节数转为 MB 字符串，保留一位小数 */
function toMB(bytes: number): string {
  return (bytes / (1024 * 1024)).toFixed(1);
}

/** 存储子项面板：库体积/阈值/清理 + 备份与恢复 */
function StoragePanel() {
  const [liveCount, setLiveCount] = useState(0);
  const [fileSizeBytes, setFileSizeBytes] = useState(0);
  const [cleanupMsg, setCleanupMsg] = useState<string | null>(null);
  const [opError, setOpError] = useState<string | null>(null);
  const [imageThresholdMB, setImageThresholdMB] = useState(DEFAULT_IMAGE_THRESHOLD_MB);
  const [passphrase, setPassphrase] = useState("");
  const [backupPath, setBackupPath] = useState<string | null>(null);
  const [backupUsedFallbackDir, setBackupUsedFallbackDir] = useState(false);
  const [backupMsg, setBackupMsg] = useState<string | null>(null);
  const [isExporting, setIsExporting] = useState(false);
  const [isImporting, setIsImporting] = useState(false);
  const fileInputRef = useRef<HTMLInputElement | null>(null);

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

  /** 导出备份：成功后展示后端回传的落盘路径，并刷新条目统计。 */
  async function handleExportBackup() {
    setOpError(null);
    setBackupMsg(null);
    setBackupPath(null);
    setBackupUsedFallbackDir(false);
    setIsExporting(true);
    try {
      const result = await exportBackup(passphrase);
      setBackupPath(result.path);
      setBackupUsedFallbackDir(result.usedFallbackDir);
      const skippedNote =
        result.skippedCount > 0
          ? `，另有 ${result.skippedCount} 条缺少判重信息未纳入`
          : "";
      setBackupMsg(`已导出 ${result.itemCount} 条到备份包${skippedNote}`);
    } catch (err) {
      setOpError(err instanceof Error ? err.message : "导出备份失败");
    } finally {
      setIsExporting(false);
    }
  }

  /** 在文件管理器中定位刚导出的备份包。 */
  async function handleRevealBackup() {
    if (backupPath === null) return;
    try {
      await revealBackupInFinder(backupPath);
    } catch (err) {
      setOpError(err instanceof Error ? err.message : "无法在文件管理器中显示备份文件");
    }
  }

  /**
   * 导入备份：先二次确认（只合并、不删除），再读文件字节交给后端解密合并。
   *
   * 读文件放在确认之后：用户取消时一个字节都不读、也不调后端。
   */
  async function handleBackupFileSelected(event: ChangeEvent<HTMLInputElement>) {
    const input = event.currentTarget;
    const file = input.files?.[0];
    // 清空选择，保证再次选同一个文件也能触发 change
    input.value = "";
    if (file === undefined) return;

    setOpError(null);
    setBackupMsg(null);

    const confirmed = window.confirm(
      "导入会把备份包里的条目合并进当前历史，不会删除任何现有条目。继续导入？"
    );
    if (!confirmed) return;

    setIsImporting(true);
    try {
      const bytes = await file.arrayBuffer();
      const result = await importBackup(arrayBufferToBase64(bytes), passphrase);
      setBackupMsg(`导入完成：新增 ${result.inserted} 条，跳过已存在 ${result.skipped} 条`);
      const cancelled = { current: false };
      await fetchStats(cancelled);
    } catch (err) {
      setOpError(err instanceof Error ? err.message : "导入备份失败");
    } finally {
      setIsImporting(false);
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
            <div className="label">单张图片阈值</div>
            <div className="desc">超过此大小的图片只保留缩略图，原图标记为过大未存</div>
          </div>
          <Select
            ariaLabel="单张图片阈值"
            value={String(imageThresholdMB)}
            options={IMAGE_THRESHOLD_SELECT_OPTIONS}
            onChange={(value) => { void handleThresholdChange(Number(value)); }}
          />
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

      <SettingGroup>
        <div className="set-row">
          <div className="grow">
            <div className="label">备份 / 恢复</div>
            <div className="desc">
              导出带口令加密的备份包（.qqbp）；换机或空库时导入即可恢复历史。导入只做合并，不删除现有条目。
            </div>
          </div>
        </div>
        <div className="set-row column">
          <label className="label" htmlFor="backup-passphrase">
            备份口令
          </label>
          <input
            id="backup-passphrase"
            className="set-input"
            type="password"
            autoComplete="new-password"
            value={passphrase}
            placeholder="导出与导入需使用同一口令"
            onChange={(e) => setPassphrase(e.target.value)}
          />
          <div className="backup-actions">
            <button
              className="btn btn-primary"
              type="button"
              onClick={() => void handleExportBackup()}
              disabled={isExporting}
            >
              {isExporting ? "导出中…" : "导出备份"}
            </button>
            <button
              className="btn"
              type="button"
              onClick={() => fileInputRef.current?.click()}
              disabled={isImporting}
            >
              {isImporting ? "导入中…" : "选择备份文件导入…"}
            </button>
            <input
              ref={fileInputRef}
              type="file"
              accept=".qqbp"
              aria-label="选择备份文件"
              hidden
              onChange={(e) => void handleBackupFileSelected(e)}
            />
          </div>
        </div>
        {backupPath !== null && (
          <div className="set-row">
            <div className="grow">
              <div className="label">备份位置</div>
              <div className="desc" style={{ wordBreak: "break-all" }}>
                {backupPath}
              </div>
              {backupUsedFallbackDir && (
                <div className="desc">
                  已回退到应用数据目录（文稿/下载目录不可用）
                </div>
              )}
            </div>
            <button className="btn" type="button" onClick={() => void handleRevealBackup()}>
              在 Finder 中显示
            </button>
          </div>
        )}
      </SettingGroup>

      {cleanupMsg !== null && (
        <div style={{ marginTop: 8, fontSize: 12, color: "var(--muted)" }}>
          {cleanupMsg}
        </div>
      )}
      {backupMsg !== null && (
        <div style={{ marginTop: 8, fontSize: 12, color: "var(--muted)" }}>
          {backupMsg}
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
