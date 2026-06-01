import SettingRow from "./SettingRow";

interface SettingToggleProps {
  label: string;
  description?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
}

/** 带开关的设置行：复用 SettingRow，右侧渲染 ARIA switch button */
function SettingToggle({
  label,
  description,
  checked,
  onChange,
  disabled,
}: SettingToggleProps) {
  return (
    <SettingRow label={label} description={description}>
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
