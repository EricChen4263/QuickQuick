import { useEffect, useState } from "react";
import {
  getProviderCredentials,
  setProviderCredentials,
  type CredentialField,
} from "../../ipc/ipc-client";

interface CredentialFormProps {
  providerId: string;
  schema: CredentialField[];
  onSaved: () => void;
}

/** 凭据配置表单：按 schema 渲染字段，保存时过滤掉空串 secret（不覆盖已有值）。 */
function CredentialForm({ providerId, schema, onSaved }: CredentialFormProps) {
  const [formValues, setFormValues] = useState<Record<string, string>>({});
  const [secretIsSet, setSecretIsSet] = useState<Set<string>>(new Set());
  const [saving, setSaving] = useState(false);
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

  return (
    <div className="credential-form">
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
      <button className="btn btn-primary" onClick={() => void handleSave()} disabled={saving}>
        保存
      </button>
      {error !== null && (
        <div role="alert" style={{ color: "var(--danger)" }}>
          {error}
        </div>
      )}
    </div>
  );
}

export default CredentialForm;
