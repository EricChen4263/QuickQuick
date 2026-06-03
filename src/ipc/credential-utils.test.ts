import { describe, it, expect } from "vitest";
import { isProviderConfigured } from "./credential-utils";
import type { CredentialField, CredentialValue } from "./ipc-client";

describe("isProviderConfigured", () => {
  it("空凭据列表（所有 required 字段都未 isSet）→ false", () => {
    const schema: CredentialField[] = [
      { key: "appId", label: "App ID", isSecret: false, required: true },
    ];
    const credentials: CredentialValue[] = [
      { key: "appId", value: null, isSet: false },
    ];

    expect(isProviderConfigured(schema, credentials)).toBe(false);
  });

  it("所有 required 字段都已 isSet → true", () => {
    const schema: CredentialField[] = [
      { key: "appId", label: "App ID", isSecret: false, required: true },
      { key: "secret", label: "Secret", isSecret: true, required: true },
    ];
    const credentials: CredentialValue[] = [
      { key: "appId", value: "my-app-id", isSet: true },
      { key: "secret", value: null, isSet: true },
    ];

    expect(isProviderConfigured(schema, credentials)).toBe(true);
  });

  it("required 字段缺少一个未 isSet → false", () => {
    const schema: CredentialField[] = [
      { key: "appId", label: "App ID", isSecret: false, required: true },
      { key: "secret", label: "Secret", isSecret: true, required: true },
    ];
    const credentials: CredentialValue[] = [
      { key: "appId", value: "my-app-id", isSet: true },
      { key: "secret", value: null, isSet: false },
    ];

    expect(isProviderConfigured(schema, credentials)).toBe(false);
  });

  it("无 required 字段的 schema → true（无需配置）", () => {
    const schema: CredentialField[] = [
      { key: "optional", label: "Optional Field", isSecret: false, required: false },
    ];
    const credentials: CredentialValue[] = [
      { key: "optional", value: null, isSet: false },
    ];

    expect(isProviderConfigured(schema, credentials)).toBe(true);
  });
});
