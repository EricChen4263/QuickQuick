import type { CredentialField, CredentialValue } from "./ipc-client";

/**
 * 判断指定 Provider 是否已完整配置凭据。
 *
 * 所有 required 字段都已 isSet 时返回 true；否则返回 false。
 * 无 required 字段的 schema（如无需 Key 的 Provider）始终返回 true。
 */
export function isProviderConfigured(
  schema: CredentialField[],
  credentials: CredentialValue[]
): boolean {
  const credentialMap = new Map(credentials.map((c) => [c.key, c]));

  return schema
    .filter((field) => field.required)
    .every((field) => credentialMap.get(field.key)?.isSet === true);
}
