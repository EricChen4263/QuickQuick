/**
 * 外观子项面板：列表排序方式、弹出面板图片高度、首次预览延迟、命中高亮样式。
 *
 * 四项都是「只影响展示」的偏好：
 * - 排序只决定行的先后，不参与去重、收藏与淘汰；
 * - 图片高度只改弹窗行内缩略图的高度；
 * - 首次预览延迟只改「面板打开后第一张预览等多久」，后续切换仍固定 200ms；
 * - 命中高亮只换 <mark> 的呈现，命中区间与搜索行为不变。
 *
 * 每行都带 tooltip 写明默认值（`title`），文案约定与 Maccy 一致：
 * 用户看一眼就知道「不动它是什么样」。
 */

import { useEffect, useState } from "react";
import { useSettingsSaveError } from "./settings-save-error";
import type { HighlightStyle, SortOrder } from "../../ipc/ipc-client";
import { Select } from "../../components/Select";
import {
  DEFAULT_IMAGE_HEIGHT_PX,
  DEFAULT_PREVIEW_FIRST_DELAY_MS,
  HIGHLIGHT_STYLE_OPTIONS,
  MAX_IMAGE_HEIGHT_PX,
  MAX_PREVIEW_FIRST_DELAY_MS,
  MIN_IMAGE_HEIGHT_PX,
  MIN_PREVIEW_FIRST_DELAY_MS,
  SORT_ORDER_OPTIONS,
  parseIntInRange,
} from "./appearance-settings";
import { useAppearanceSettings } from "./useAppearanceSettings";
import PanelHeader from "./PanelHeader";
import SettingGroup from "./SettingGroup";
import SettingRow from "./SettingRow";

/** 排序方式 tooltip：必须含默认值。 */
const SORT_ORDER_TOOLTIP =
  "默认：最近复制。最近复制按最近一次复制时间倒序；首次复制按首次入库时间倒序，再次复制不改变位置。只影响列表展示顺序，不改变去重与淘汰。";

/** 图片高度 tooltip：必须含默认值。 */
const IMAGE_HEIGHT_TOOLTIP = `默认：${DEFAULT_IMAGE_HEIGHT_PX}px（范围 ${MIN_IMAGE_HEIGHT_PX}–${MAX_IMAGE_HEIGHT_PX}px）。只影响弹出面板列表里的图片缩略图高度，主窗口列表不受影响。`;

/** 首次预览延迟 tooltip：必须含默认值。 */
const PREVIEW_DELAY_TOOLTIP = `默认：${DEFAULT_PREVIEW_FIRST_DELAY_MS}ms（范围 ${MIN_PREVIEW_FIRST_DELAY_MS}–${MAX_PREVIEW_FIRST_DELAY_MS}ms）。面板打开后首个预览的等待时长；之后切换条目的 200ms 防抖固定不变。`;

/** 命中高亮 tooltip：必须含默认值。 */
const HIGHLIGHT_STYLE_TOOLTIP =
  "默认：加粗（对齐 Maccy 的默认档）。可选 加粗 / 背景色 / 斜体 / 下划线，四档都只改命中片段的呈现，命中区间与搜索行为不变。";

/** 外观子项面板 */
function AppearancePanel() {
  const {
    sortOrder,
    imageHeightPx,
    previewFirstDelayMs,
    highlightStyle,
    setSortOrder,
    setImageHeightPx,
    setPreviewFirstDelayMs,
    setHighlightStyle,
  } = useAppearanceSettings();

  // 输入框文本与「已生效值」分开：允许用户自由编辑（含清空、输错），
  // 提交失败时把文本回滚到已生效值，避免「显示的」和「真正生效的」悄悄分叉。
  const [imageHeightInput, setImageHeightInput] = useState(String(DEFAULT_IMAGE_HEIGHT_PX));
  const [previewDelayInput, setPreviewDelayInput] = useState(
    String(DEFAULT_PREVIEW_FIRST_DELAY_MS),
  );
  // 保存失败统一走页面级错误条（后端中文原文，含修复指引）；这里的 message 只留校验类提示
  const { runSave } = useSettingsSaveError();
  const [message, setMessage] = useState<string | null>(null);

  // 读到（或成功保存）后把输入框同步成已生效值
  useEffect(() => {
    setImageHeightInput(String(imageHeightPx));
  }, [imageHeightPx]);
  useEffect(() => {
    setPreviewDelayInput(String(previewFirstDelayMs));
  }, [previewFirstDelayMs]);

  async function handleSortOrderChange(value: string): Promise<void> {
    setMessage(null);
    const result = await runSave(() => setSortOrder(value as SortOrder));
    if (!result.ok) console.error("排序方式保存失败:", result.error);
  }

  async function handleHighlightStyleChange(value: string): Promise<void> {
    setMessage(null);
    const result = await runSave(() => setHighlightStyle(value as HighlightStyle));
    if (!result.ok) console.error("命中高亮样式保存失败:", result.error);
  }

  /** 提交图片高度：非法输入不调后端，把输入框回滚到已生效值并提示范围。 */
  async function commitImageHeight(): Promise<void> {
    const parsed = parseIntInRange(imageHeightInput, MIN_IMAGE_HEIGHT_PX, MAX_IMAGE_HEIGHT_PX);
    if (parsed === null) {
      setMessage(
        `图片高度需为 ${MIN_IMAGE_HEIGHT_PX}–${MAX_IMAGE_HEIGHT_PX} 之间的整数（px）`,
      );
      setImageHeightInput(String(imageHeightPx));
      return;
    }
    setMessage(null);
    if (parsed === imageHeightPx) {
      // 顺手归一化文本（"024" → "24"）
      setImageHeightInput(String(imageHeightPx));
      return;
    }
    const result = await runSave(() => setImageHeightPx(parsed));
    if (!result.ok) {
      console.error("图片高度保存失败:", result.error);
      setImageHeightInput(String(imageHeightPx));
    }
  }

  /** 提交首次预览延迟：规则与图片高度一致。 */
  async function commitPreviewDelay(): Promise<void> {
    const parsed = parseIntInRange(
      previewDelayInput,
      MIN_PREVIEW_FIRST_DELAY_MS,
      MAX_PREVIEW_FIRST_DELAY_MS,
    );
    if (parsed === null) {
      setMessage(
        `首次预览延迟需为 ${MIN_PREVIEW_FIRST_DELAY_MS}–${MAX_PREVIEW_FIRST_DELAY_MS} 之间的整数（毫秒）`,
      );
      setPreviewDelayInput(String(previewFirstDelayMs));
      return;
    }
    setMessage(null);
    if (parsed === previewFirstDelayMs) {
      setPreviewDelayInput(String(previewFirstDelayMs));
      return;
    }
    const result = await runSave(() => setPreviewFirstDelayMs(parsed));
    if (!result.ok) {
      console.error("首次预览延迟保存失败:", result.error);
      setPreviewDelayInput(String(previewFirstDelayMs));
    }
  }

  return (
    <div>
      <PanelHeader title="外观" subtitle="列表顺序、图片与预览的展示偏好。" />
      <SettingGroup>
        <SettingRow
          label="排序方式"
          description="最近复制：最近一次复制的排最前；首次复制：按首次入库时间倒序，再次复制不跳位"
          tooltip={SORT_ORDER_TOOLTIP}
        >
          <Select
            ariaLabel="排序方式"
            value={sortOrder}
            options={SORT_ORDER_OPTIONS}
            onChange={(value) => {
              void handleSortOrderChange(value);
            }}
          />
        </SettingRow>

        <SettingRow
          label="弹出面板图片高度"
          description={`行内缩略图高度，范围 ${MIN_IMAGE_HEIGHT_PX}–${MAX_IMAGE_HEIGHT_PX}px（对齐 Maccy）`}
          tooltip={IMAGE_HEIGHT_TOOLTIP}
        >
          <input
            id="image-height"
            className="set-input short-num num"
            type="number"
            min={MIN_IMAGE_HEIGHT_PX}
            max={MAX_IMAGE_HEIGHT_PX}
            step={1}
            aria-label="弹出面板图片高度"
            value={imageHeightInput}
            onChange={(e) => setImageHeightInput(e.target.value)}
            onBlur={() => void commitImageHeight()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void commitImageHeight();
            }}
          />
        </SettingRow>

        <SettingRow
          label="首次预览延迟"
          description={`面板打开后首个预览等多久，范围 ${MIN_PREVIEW_FIRST_DELAY_MS}–${MAX_PREVIEW_FIRST_DELAY_MS}ms；后续切换固定 200ms`}
          tooltip={PREVIEW_DELAY_TOOLTIP}
        >
          <input
            id="preview-first-delay"
            className="set-input short-num num"
            type="number"
            min={MIN_PREVIEW_FIRST_DELAY_MS}
            max={MAX_PREVIEW_FIRST_DELAY_MS}
            step={100}
            aria-label="首次预览延迟"
            value={previewDelayInput}
            onChange={(e) => setPreviewDelayInput(e.target.value)}
            onBlur={() => void commitPreviewDelay()}
            onKeyDown={(e) => {
              if (e.key === "Enter") void commitPreviewDelay();
            }}
          />
        </SettingRow>

        <SettingRow
          label="命中高亮样式"
          description="搜索命中片段的呈现方式"
          tooltip={HIGHLIGHT_STYLE_TOOLTIP}
        >
          <Select
            ariaLabel="命中高亮样式"
            value={highlightStyle}
            options={HIGHLIGHT_STYLE_OPTIONS}
            onChange={(value) => {
              void handleHighlightStyleChange(value);
            }}
          />
        </SettingRow>
      </SettingGroup>
      {message !== null && (
        <div role="status" style={{ marginTop: 8, fontSize: 12, color: "var(--muted)" }}>
          {message}
        </div>
      )}
    </div>
  );
}

export default AppearancePanel;
