/** 备份文件读取与编码辅助。 */

/** 分块大小：一次转换 32 KiB 字节，避免 `String.fromCharCode` 展开大数组时爆栈。 */
const CHUNK_SIZE = 0x8000;

/**
 * 把文件字节编码成 base64 字符串，供导入命令传递给后端。
 *
 * 为什么分块：`String.fromCharCode(...bytes)` 一次性展开几十万个小整数会触发
 * 「Maximum call stack size exceeded」，备份包里的图片字节很容易到这个量级。
 *
 * @param buffer - 文件字节（`File.arrayBuffer()` 的结果）
 * @returns 标准字母表 base64 字符串
 */
export function arrayBufferToBase64(buffer: ArrayBuffer): string {
  const bytes = new Uint8Array(buffer);
  let binary = "";
  for (let offset = 0; offset < bytes.length; offset += CHUNK_SIZE) {
    const chunk = bytes.subarray(offset, offset + CHUNK_SIZE);
    binary += String.fromCharCode(...chunk);
  }
  return btoa(binary);
}
