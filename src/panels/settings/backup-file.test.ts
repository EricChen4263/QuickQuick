import { describe, expect, it } from "vitest";
import { arrayBufferToBase64 } from "./backup-file";

/** 把字节数组包装成 ArrayBuffer。 */
function toBuffer(bytes: number[]): ArrayBuffer {
  return new Uint8Array(bytes).buffer;
}

describe("arrayBufferToBase64", () => {
  it("空字节编码为空串", () => {
    expect(arrayBufferToBase64(toBuffer([]))).toBe("");
  });

  it("三字节编码为对应的标准 base64（QUJD = \"ABC\"）", () => {
    expect(arrayBufferToBase64(toBuffer([65, 66, 67]))).toBe("QUJD");
  });

  it("跨分块边界仍能完整编码（分块拼接不丢字节、不爆栈）", () => {
    // Arrange: 0x8000 分块边界前后各留一段，共 100000 字节全为 0x41（"A"）
    const size = 100_000;
    const bytes = new Array<number>(size).fill(65);

    // Act
    const encoded = arrayBufferToBase64(toBuffer(bytes));

    // Assert: base64 长度 = ceil(n/3)*4，且解回来仍是 100000 个 "A"
    expect(encoded).toHaveLength(Math.ceil(size / 3) * 4);
    expect(atob(encoded)).toBe("A".repeat(size));
  });
});
