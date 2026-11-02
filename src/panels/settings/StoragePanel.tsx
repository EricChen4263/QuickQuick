import { useEffect, useState, useCallback, useRef } from "react";
import type { ChangeEvent } from "react";
import {
  getStorageStats,
  cleanupHistory,
  getImageThreshold,
  setImageThreshold,
  getMaxHistoryItems,
  setMaxHistoryItems,
  MAX_HISTORY_ITEMS,
  MIN_HISTORY_ITEMS,
  exportBackup,
  importBackup,
  revealBackupInFinder,
} from "../../ipc/ipc-client";
import { Select } from "../../components/Select";
import type { SelectOption } from "../../components/Select";
import { arrayBufferToBase64 } from "./backup-file";
import PanelHeader from "./PanelHeader";
import { useSettingsSaveError } from "./settings-save-error";
import SettingGroup from "./SettingGroup";

/** 存储上限：500 MB（字节） */
const MAX_BYTES = 500 * 1024 * 1024;

/** 默认单张图片阈值（MB） */
const DEFAULT_IMAGE_THRESHOLD_MB = 20;

/** 历史条目上限的默认值（与后端 AppSettings 默认值一致，仅用于 IPC 未返回时占位） */
const DEFAULT_HISTORY_ITEMS = 500;

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

/**
 * 校验历史条目上限的输入文本，合法则返回条数，否则返回 null。
 *
 * 只接受十进制整数：`"2.5"`、`"1e3"`、`""`、`"abc"` 全部判非法。
 * `Number.parseInt("2.5")` 会得到 2，那种「悄悄取整」会让用户以为自己设了 2.5 条，
 * 实际生效的是别的数字，所以这里用正则先卡形状，再判区间。
 */
function parseHistoryLimitInput(raw: string): number | null {
  const trimmed = raw.trim();
  if (!/^\d+$/.test(trimmed)) return null;
  const value = Number(trimmed);
  if (value < MIN_HISTORY_ITEMS || value > MAX_HISTORY_ITEMS) return null;
  return value;
}

/** 存储子项面板：图片占用/库文件体积/阈值/清理 + 备份与恢复 */
function StoragePanel() {
  const [liveCount, setLiveCount] = useState(0);
  // 两个占用口径分开存：imageBytes 受 500 MB 上限约束（进度条用它），
  // fileSizeBytes 是磁盘占用，二者不可互相替代。
  const [imageBytes, setImageBytes] = useState(0);
  const [fileSizeBytes, setFileSizeBytes] = useState(0);
  const [cleanupMsg, setCleanupMsg] = useState<string | null>(null);
  // 写失败统一走页面级错误条（后端中文原文，按来源隔离）；
  // 但**读**失败（统计/加载）不是保存失败，用本地状态在面板内提示，不污染错误条（J）。
  const { reportSaveError, clearSaveError } = useSettingsSaveError();
  const [statsLoadError, setStatsLoadError] = useState<string | null>(null);
  const [imageThresholdMB, setImageThresholdMB] = useState(DEFAULT_IMAGE_THRESHOLD_MB);
  // 历史条目上限：inputText 是输入框当前文本，savedItems 是后端已确认生效的值。
  // 两者分开才能允许用户自由编辑（含清空、输错），而不让「输入框显示的」和
  // 「真正生效的」悄悄分叉——提交失败时 inputText 回滚到 savedItems。
  const [historyInput, setHistoryInput] = useState(String(DEFAULT_HISTORY_ITEMS));
  const [savedHistoryItems, setSavedHistoryItems] = useState(DEFAULT_HISTORY_ITEMS);
  const [historyMsg, setHistoryMsg] = useState<string | null>(null);
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
      setImageBytes(stats.imageBytes);
      setFileSizeBytes(stats.fileSizeBytes);
      setStatsLoadError(null);
    } catch (err) {
      if (cancelled.current) return;
      console.error("存储统计加载失败:", err);
      setStatsLoadError("存储统计加载失败，请稍后重试");
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

    const loadHistoryLimit = async () => {
      try {
        const items = await getMaxHistoryItems();
        if (cancelled.current) return;
        setSavedHistoryItems(items);
        setHistoryInput(String(items));
      } catch {
        if (cancelled.current) return;
        console.error("历史条目上限加载失败，使用默认值 500 条");
      }
    };
    void loadHistoryLimit();

    return () => {
      cancelled.current = true;
    };
  }, [fetchStats]);

  async function handleThresholdChange(mb: number) {
    try {
      await setImageThreshold(mb * 1024 * 1024);
      setImageThresholdMB(mb);
      clearSaveError("storage.imageThreshold");
    } catch (err) {
      console.error("设置图片阈值失败:", err);
      reportSaveError("storage.imageThreshold", err);
    }
  }

  /**
   * 提交历史条目上限（失焦或回车触发）。
   *
   * 四条约束：
   * - 值没变就不打后端：失焦会无条件触发本函数，标签页里路过一次输入框
   *   不该换来一次 settings.json 落盘加一次淘汰查询。
   * - 非法输入（非整数 / 越界）**不调后端**，把输入框回滚到已生效值并给出提示：
   *   输入框显示的值必须等于真正生效的值，否则用户会误以为设置成功了。
   * - 提交成功后用后端返回的条数报告本轮淘汰量：上限调小会立刻删条目，
   *   不告诉用户「刚才删了几条」等于偷偷改了他的历史。
   * - 提交后重拉统计，让上面的条目数随之更新。
   */
  async function commitHistoryLimit() {
    const parsed = parseHistoryLimitInput(historyInput);
    if (parsed === null) {
      setHistoryMsg(`上限需为 ${MIN_HISTORY_ITEMS}–${MAX_HISTORY_ITEMS} 之间的整数`);
      setHistoryInput(String(savedHistoryItems));
      return;
    }
    if (parsed === savedHistoryItems) {
      // 顺手归一化文本（"050" → "50"）：否则输入框会留着一个与已生效值"看起来不同"的串
      setHistoryInput(String(savedHistoryItems));
      return;
    }

    try {
      const evicted = await setMaxHistoryItems(parsed);
      clearSaveError("storage.historyLimit");
      setSavedHistoryItems(parsed);
      setHistoryInput(String(parsed));
      setHistoryMsg(evicted > 0 ? `已按新上限清理 ${evicted} 条最旧的非收藏记录` : null);
      const cancelled = { current: false };
      await fetchStats(cancelled);
    } catch (err) {
      reportSaveError("storage.historyLimit", err);
      setHistoryInput(String(savedHistoryItems));
    }
  }

  async function handleCleanup() {
    setCleanupMsg(null);
    try {
      const result = await cleanupHistory();
      clearSaveError("storage.cleanup");
      const cancelled = { current: false };
      await fetchStats(cancelled);
      setCleanupMsg(`已清理 ${result.softDeleted + result.purged} 条`);
    } catch (err) {
      reportSaveError("storage.cleanup", err);
    }
  }

  /** 导出备份：成功后展示后端回传的落盘路径，并刷新条目统计。 */
  async function handleExportBackup() {
    setBackupMsg(null);
    setBackupPath(null);
    setBackupUsedFallbackDir(false);
    setIsExporting(true);
    try {
      const result = await exportBackup(passphrase);
      clearSaveError("storage.export");
      setBackupPath(result.path);
      setBackupUsedFallbackDir(result.usedFallbackDir);
      const skippedNote =
        result.skippedCount > 0
          ? `，另有 ${result.skippedCount} 条缺少判重信息未纳入`
          : "";
      setBackupMsg(`已导出 ${result.itemCount} 条到备份包${skippedNote}`);
    } catch (err) {
      reportSaveError("storage.export", err);
    } finally {
      setIsExporting(false);
    }
  }

  /** 在文件管理器中定位刚导出的备份包。 */
  async function handleRevealBackup() {
    if (backupPath === null) return;
    try {
      await revealBackupInFinder(backupPath);
      clearSaveError("storage.reveal");
    } catch (err) {
      reportSaveError("storage.reveal", err);
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

    setBackupMsg(null);

    const confirmed = window.confirm(
      "导入会把备份包里的条目合并进当前历史，不会删除任何现有条目。继续导入？"
    );
    if (!confirmed) return;

    setIsImporting(true);
    try {
      const bytes = await file.arrayBuffer();
      const result = await importBackup(arrayBufferToBase64(bytes), passphrase);
      clearSaveError("storage.import");
      setBackupMsg(`导入完成：新增 ${result.inserted} 条，跳过已存在 ${result.skipped} 条`);
      const cancelled = { current: false };
      await fetchStats(cancelled);
    } catch (err) {
      reportSaveError("storage.import", err);
    } finally {
      setIsImporting(false);
    }
  }

  // 进度条只能取图片占用：上限约束的就是这个量，同源才能让清理后条真的下降。
  const meterWidth = Math.min((imageBytes / MAX_BYTES) * 100, 100);

  return (
    <div>
      <PanelHeader title="存储" subtitle="加密库 quickquick.db · 分级清理，收藏永远豁免。" />
      {statsLoadError !== null && (
        <div role="alert" className="set-inline-error">
          {statsLoadError}
        </div>
      )}
      <SettingGroup>
        <div className="set-row">
          <div className="grow">
            <div className="label title-4">图片占用</div>
            <div className="desc caption-5">
              受 500 MB 上限约束；进度条与自动清理的阈值判据取的是同一个量
            </div>
            <div className="meter" aria-hidden="true">
              <i style={{ width: `${meterWidth}%` }} />
            </div>
            <div className="meter-legend caption-5 mono">
              <span>{toMB(imageBytes)} MB 已用</span>
              <span>图片占用上限 500 MB</span>
            </div>
          </div>
        </div>
        <div className="set-row">
          <div className="grow">
            <div className="label title-4">库文件体积</div>
            <div className="desc caption-5">
              {toMB(fileSizeBytes)} MB · 磁盘实际占用，与上面的图片占用不是同一个量：还含文本条目、
              索引与未回收页，SQLite 删除只把页标记为可复用，文件在 VACUUM 前不会缩小
            </div>
          </div>
        </div>
        <div className="set-row">
          <div className="grow">
            <div className="label title-4">条目数</div>
            <div className="desc caption-5">{liveCount} 条活跃记录（不含软删）</div>
          </div>
        </div>
        <div
          className="set-row"
          title={`默认：${DEFAULT_HISTORY_ITEMS} 条。超过此数量的非收藏条目会在入库时自动淘汰最旧的；收藏项不计数也不删除。`}
        >
          <div className="grow">
            <label className="label title-4" htmlFor="history-limit">
              历史条目上限
            </label>
            <div className="desc caption-5">
              超过此数量的非收藏条目会在入库时自动淘汰最旧的；收藏项不计数也不删除。
              范围 {MIN_HISTORY_ITEMS}–{MAX_HISTORY_ITEMS} 条，改小会立即生效。
            </div>
            {historyMsg !== null && (
              <div className="desc caption-5" role="status">
                {historyMsg}
              </div>
            )}
          </div>
          <input
            id="history-limit"
            className="set-input short-num num body-6"
            type="number"
            min={MIN_HISTORY_ITEMS}
            max={MAX_HISTORY_ITEMS}
            step={1}
            aria-label="历史条目上限"
            value={historyInput}
            onChange={(e) => setHistoryInput(e.target.value)}
            onBlur={() => void commitHistoryLimit()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void commitHistoryLimit();
            }}
          />
        </div>
        <div
          className="set-row"
          title="默认：500 MB，固定不可配置（与后端 image::CleanupPolicy::default 同源）。超出后按分级清理：先删大图原图保缩略图，再整条清最旧非收藏。"
        >
          <div className="grow">
            <div className="label title-4">图片占用上限</div>
            <div className="desc caption-5">超出按分级清理：先删大图原图保缩略图，再整条清最旧非收藏</div>
          </div>
          <span className="kbd-combo caption-4 mono">
            <kbd>500 MB</kbd>
          </span>
        </div>
        <div
          className="set-row"
          title={`默认：${DEFAULT_IMAGE_THRESHOLD_MB} MB。超过此大小的图片只保留缩略图，原图标记为过大未存；可选档位 ${IMAGE_THRESHOLD_OPTIONS.join(" / ")} MB。`}
        >
          <div className="grow">
            <div className="label title-4">单张图片阈值</div>
            <div className="desc caption-5">超过此大小的图片只保留缩略图，原图标记为过大未存</div>
          </div>
          <Select
            ariaLabel="单张图片阈值"
            value={String(imageThresholdMB)}
            options={IMAGE_THRESHOLD_SELECT_OPTIONS}
            onChange={(value) => { void handleThresholdChange(Number(value)); }}
          />
        </div>
        <div
          className="set-row"
          title="一次性操作，无默认值。当场清理非收藏条目；收藏项永远豁免。"
        >
          <div className="grow">
            <div className="label title-4">立即清理非收藏历史</div>
            <div className="desc caption-5">物理清除已软删条目（墓碑 GC）</div>
          </div>
          <button className="btn button-sm" type="button" onClick={() => { void handleCleanup(); }}>
            清理…
          </button>
        </div>
      </SettingGroup>

      <SettingGroup>
        <div
          className="set-row"
          title="一次性操作，无默认值。导出为口令加密的可移植包（含条目与图片）；导入时会跳过已存在的条目。"
        >
          <div className="grow">
            <div className="label title-4">备份 / 恢复</div>
            <div className="desc caption-5">
              导出带口令加密的备份包（.qqbp）；换机或空库时导入即可恢复历史。导入只做合并，不删除现有条目。
            </div>
          </div>
        </div>
        <div className="set-row column">
          <label className="label title-4" htmlFor="backup-passphrase">
            备份口令
          </label>
          <input
            id="backup-passphrase"
            className="set-input body-6"
            type="password"
            autoComplete="new-password"
            value={passphrase}
            placeholder="导出与导入需使用同一口令"
            onChange={(e) => setPassphrase(e.target.value)}
          />
          <div className="backup-actions">
            <button
              className="btn btn-primary button-sm"
              type="button"
              onClick={() => void handleExportBackup()}
              disabled={isExporting}
            >
              {isExporting ? "导出中…" : "导出备份"}
            </button>
            <button
              className="btn button-sm"
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
          <div
            className="set-row"
            title="默认：优先「文稿」目录，其次「下载」目录；两者都不可用时回退到应用数据目录。"
          >
            <div className="grow">
              <div className="label title-4">备份位置</div>
              <div className="desc caption-5 set-path">
                {backupPath}
              </div>
              {backupUsedFallbackDir && (
                <div className="desc caption-5">
                  已回退到应用数据目录（文稿/下载目录不可用）
                </div>
              )}
            </div>
            <button className="btn button-sm" type="button" onClick={() => void handleRevealBackup()}>
              在 Finder 中显示
            </button>
          </div>
        )}
      </SettingGroup>

      {cleanupMsg !== null && (
        <div className="caption-5 set-feedback">{cleanupMsg}</div>
      )}
      {backupMsg !== null && (
        <div className="caption-5 set-feedback">{backupMsg}</div>
      )}
    </div>
  );
}

export default StoragePanel;
