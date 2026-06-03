import { useEffect, useState } from "react";
import {
  getProviderCredentials,
  setProviderCredentials,
  deleteProviderCredentials,
  type CredentialField,
} from "../../ipc/ipc-client";

interface CredentialFormProps {
  providerId: string;
  schema: CredentialField[];
  onSaved: () => void;
  /** 该 provider 是否已配置完整凭据（由父组件传入，复用 configuredIds 状态）。 */
  isConfigured?: boolean;
}

/** 凭据配置表单：按 schema 渲染字段，保存时过滤掉空串 secret（不覆盖已有值）。
 *
 * 已配置时在顶部显示提示并提供「清除已存密钥」按钮，方便用户重置 keychain 残留凭据。
 */
function CredentialForm({ providerId, schema, onSaved, isConfigured = false }: CredentialFormProps) {
  const [formValues, setFormValues] = useState<Record<string, string>>({});
  const [secretIsSet, setSecretIsSet] = useState<Set<string>>(new Set());
  const [saving, setSaving] = useState(false);
  const [clearing, setClearing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const cancelled = { current: false };

    async function loadCredentials() {
      try {
        const credentials = await getProviderCredentials(providerId);
        if (cancelled.current) return;

        const initialValues: Record<string, string> = {};
        const setKeys = new Set<string>();

        for (const cred of credentials) {
          if (cred.isSet) {
            setKeys.add(cred.key);
          }
          if (cred.value !== null) {
            initialValues[cred.key] = cred.value;
          }
        }

        setFormValues(initialValues);
        setSecretIsSet(setKeys);
      } catch {
        if (cancelled.current) return;
        setError("加载凭据失败");
      }
    }

    void loadCredentials();
    return () => {
      cancelled.current = true;
    };
  }, [providerId]);

  async function handleSave() {
    setSaving(true);
    setError(null);

    const filtered: Record<string, string> = {};
    for (const field of schema) {
      const value = formValues[field.key] ?? "";
      if (field.isSecret && value === "") {
        continue;
      }
      filtered[field.key] = value;
    }

    try {
      await setProviderCredentials(providerId, filtered);
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : "保存失败");
    } finally {
      setSaving(false);
    }
  }

  async function handleClear() {
    if (!window.confirm("确定清除该翻译源已保存的密钥？")) {
      return;
    }
    setClearing(true);
    setError(null);
    try {
      await deleteProviderCredentials(providerId);
      setFormValues({});
      setSecretIsSet(new Set());
      onSaved();
    } catch (err) {
      setError(err instanceof Error ? err.message : "清除失败");
    } finally {
      setClearing(false);
    }
  }

  return (
    <div className="credential-form">
      {isConfigured && (
        <div className="credential-configured-hint">
          已配置 · 重新填写下方字段将覆盖
        </div>
      )}
      {schema.map((field) => (
        <div key={field.key} className="credential-field">
          <label htmlFor={`cred-${field.key}`}>{field.label}</label>
          <input
            id={`cred-${field.key}`}
            className="set-input"
            type={field.isSecret ? "password" : "text"}
            value={formValues[field.key] ?? ""}
            placeholder={
              field.isSecret && secretIsSet.has(field.key)
                ? "已设置（留空不修改）"
                : ""
            }
            onChange={(e) =>
              setFormValues((prev) => ({ ...prev, [field.key]: e.target.value }))
            }
          />
        </div>
      ))}
      <div className="credential-actions">
        <button className="btn btn-primary" onClick={() => void handleSave()} disabled={saving}>
          保存
        </button>
        {isConfigured && (
          <button
            className="btn btn-clear-cred"
            onClick={() => void handleClear()}
            disabled={clearing}
          >
            清除已存密钥
          </button>
        )}
      </div>
      {error !== null && (
        <div role="alert" style={{ color: "var(--danger)" }}>
          {error}
        </div>
      )}
    </div>
  );
}

export default CredentialForm;
