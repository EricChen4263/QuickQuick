import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const settingsCssPath = resolve(process.cwd(), "src/panels/settings/settings.css");

function readSettingsCss(): string {
  return readFileSync(settingsCssPath, "utf-8");
}

describe("settings.css layout contract", () => {
  it("keeps the secondary settings menu compact", () => {
    const css = readSettingsCss();

    expect(css).toMatch(/grid-template-columns:\s*128px\s+minmax\(0,\s*1fr\)/);
  });
});
