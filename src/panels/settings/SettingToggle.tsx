import SettingRow from "./SettingRow";

interface SettingToggleProps {
  label: string;
  description?: string;
  /** 悬停 tooltip：**必须写明该项默认值**（约定见 SettingRow） */
  tooltip?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

/** 带开关的设置行：复用 SettingRow，右侧渲染 ARIA switch button */
function SettingToggle({
  label,
  description,
  tooltip,
  checked,
  onChange,
  disabled,
}: SettingToggleProps) {
  return (
    <SettingRow label={label} description={description} tooltip={tooltip}>
      <button
        className="switch"
        role="switch"
        type="button"
        aria-checked={checked}
        aria-label={label}
        disabled={disabled}
        onClick={() => onChange(!checked)}
      />
    </SettingRow>
  );
}

export default SettingToggle;
