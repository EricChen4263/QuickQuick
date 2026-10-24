#!/usr/bin/env node
// 后端「写完没接线」候选清单：统计 src-tauri/src 下 pub 条目在三个引用面上的出现次数。
//
// 为什么不用 cargo-udeps：它要 nightly 工具链，且只覆盖「未使用的 crate 依赖」，
// 对本项目最典型的事故形态（模块写好了、pub fn 也在，但生产路径零调用点）无能为力。
// rustc 的 dead_code lint 同样看不见：pub 条目在 lib crate 里天然对外可见，
// 只有 pub(crate)/私有条目才会被它报出来。
//
// 三个引用面：
//   prod-other  本文件以外的生产代码（src-tauri/src/**）——真正算「接线」的面
//   prod-own    同文件内其它位置（impl 内部互调）；对 pub API 而言不等于接线
//   tests       src-tauri/tests/** 的集成测试
//
// 同时剔除注释与字符串字面量后再计数：否则「注释里提到过某个名字」会把孤儿伪装成接线。
//
// 只读脚本，正常完成时退出码恒定 0（只有脚本自身出错才非 0）。它产出的是候选清单，
// 不是判决——pub 条目是否为死代码需要人看语义（IPC 命令、trait 实现、预留 API
// 都可能合理零引用）。

import { readdir, readFile } from "node:fs/promises";
import { join, relative } from "node:path";

const SRC_ROOT = "src-tauri/src";
const TEST_ROOT = "src-tauri/tests";

/** 需要人看的候选项分类，按严重度从高到低。 */
const CATEGORY_ORPHAN = "零引用";
const CATEGORY_TEST_ONLY = "仅测试引用";
const CATEGORY_SAME_FILE_ONLY = "仅本文件引用";

/**
 * 递归收集指定后缀的文件路径。
 * @param {string} dir 起始目录（相对仓库根）
 * @param {string} suffix 文件后缀过滤
 * @returns {Promise<string[]>} 排序后的文件路径
 */
async function collectFiles(dir, suffix) {
  const found = [];
  const walk = async (current) => {
    const entries = await readdir(current, { withFileTypes: true });
    for (const entry of entries) {
      const full = join(current, entry.name);
      if (entry.isDirectory()) {
        await walk(full);
      } else if (entry.name.endsWith(suffix)) {
        found.push(full);
      }
    }
  };
  await walk(dir);
  return found.sort();
}

/**
 * 抹掉 Rust 源码里的注释与字符串字面量，保留代码标识符。
 * 保留原始换行数量，便于把偏移映射回行号。
 * @param {string} code 原始源码
 * @returns {string} 等长结构近似、仅剩代码的文本
 */
function stripCommentsAndStrings(code) {
  let out = "";
  let index = 0;
  while (index < code.length) {
    const char = code[index];
    const next = code[index + 1];
    if (char === "/" && next === "/") {
      while (index < code.length && code[index] !== "\n") {
        out += " ";
        index += 1;
      }
      continue;
    }
    if (char === "/" && next === "*") {
      // Rust 允许嵌套块注释，必须计深度，否则第二层 `/` 会提前结束。
      let depth = 1;
      out += "  ";
      index += 2;
      while (index < code.length && depth > 0) {
        if (code[index] === "/" && code[index + 1] === "*") {
          depth += 1;
          out += "  ";
          index += 2;
          continue;
        }
        if (code[index] === "*" && code[index + 1] === "/") {
          depth -= 1;
          out += "  ";
          index += 2;
          continue;
        }
        out += code[index] === "\n" ? "\n" : " ";
        index += 1;
      }
      continue;
    }
    // 原始字符串 r"..." / r#"..."# / br#"..."#：井号数量决定终止形态。
    if (char === "r" && (next === '"' || next === "#")) {
      let cursor = index + 1;
      let hashes = 0;
      while (code[cursor] === "#") {
        hashes += 1;
        cursor += 1;
      }
      if (code[cursor] === '"') {
        const terminator = `"${"#".repeat(hashes)}`;
        const end = code.indexOf(terminator, cursor + 1);
        const stop = end === -1 ? code.length : end + terminator.length;
        out += code.slice(index, stop).replace(/[^\n]/g, " ");
        index = stop;
        continue;
      }
    }
    if (char === '"') {
      let cursor = index + 1;
      while (cursor < code.length && code[cursor] !== '"') {
        cursor += code[cursor] === "\\" ? 2 : 1;
      }
      const stop = Math.min(cursor + 1, code.length);
      out += code.slice(index, stop).replace(/[^\n]/g, " ");
      index = stop;
      continue;
    }
    out += char;
    index += 1;
  }
  return out;
}

/**
 * 找出源码里的 pub 条目声明。
 * @param {string} code 已抹掉注释与字符串的源码
 * @returns {Array<{kind: string, name: string, offset: number, isIpcCommand: boolean}>}
 */
function findPubDeclarations(code) {
  const declarationPattern =
    /^[ \t]*pub[ \t]+(?:async[ \t]+|unsafe[ \t]+|const[ \t]+|extern[ \t]+)*\b(fn|struct|enum|union|trait|type|const|static|mod)\b[ \t]+([A-Za-z_][A-Za-z0-9_]*)/gm;
  const declarations = [];
  let match = declarationPattern.exec(code);
  while (match !== null) {
    const offset = match.index;
    // #[tauri::command] 的函数由前端按名字字符串经 IPC 调用，代码里没有标识符引用，
    // 这类不是死代码，必须按属性识别后排除。
    // 只看「上一个条目结束之后到本声明之间」的属性尾部，避免把上一个函数的属性误算到当前项。
    const preceding = code.slice(Math.max(0, offset - 400), offset);
    const lastItemEnd = Math.max(preceding.lastIndexOf("}"), preceding.lastIndexOf(";"));
    const isIpcCommand = /#\[tauri::command/.test(preceding.slice(lastItemEnd + 1));
    declarations.push({
      kind: match[1],
      name: match[2],
      offset,
      isIpcCommand,
    });
    match = declarationPattern.exec(code);
  }
  return declarations;
}

/**
 * 统计名字在文本里的整词出现次数。
 * @param {string} text 待统计文本
 * @param {string} name 标识符
 * @returns {number} 出现次数
 */
function countWord(text, name) {
  const pattern = new RegExp(`\\b${name}\\b`, "g");
  return (text.match(pattern) || []).length;
}

/**
 * 把偏移换算成 1 起始的行号。
 * @param {string} text 源码
 * @param {number} offset 字符偏移
 * @returns {number} 行号
 */
function offsetToLine(text, offset) {
  let line = 1;
  for (let index = 0; index < offset && index < text.length; index += 1) {
    if (text[index] === "\n") line += 1;
  }
  return line;
}

/**
 * 主流程：扫描、计数、分类、打印。
 * @returns {Promise<number>} 退出码（恒 0）
 */
async function main() {
  const srcFiles = await collectFiles(SRC_ROOT, ".rs");
  const testFiles = await collectFiles(TEST_ROOT, ".rs");

  const strippedByFile = new Map();
  for (const file of [...srcFiles, ...testFiles]) {
    const raw = await readFile(file, "utf8");
    strippedByFile.set(file, stripCommentsAndStrings(raw));
  }

  const prodText = srcFiles.map((file) => strippedByFile.get(file)).join("\n");
  const testText = testFiles.map((file) => strippedByFile.get(file)).join("\n");

  let allowDeadCodeCount = 0;
  for (const file of srcFiles) {
    allowDeadCodeCount += (strippedByFile.get(file).match(/allow\(dead_code/g) || []).length;
  }

  const rows = [];
  for (const file of srcFiles) {
    const raw = await readFile(file, "utf8");
    const code = strippedByFile.get(file);
    for (const declaration of findPubDeclarations(code)) {
      if (declaration.isIpcCommand) continue;
      const occurrencesInAllProduction = countWord(prodText, declaration.name);
      const occurrencesInOwnFile = countWord(code, declaration.name);
      const occurrencesInTests = countWord(testText, declaration.name);
      // 声明本身占一次；同文件互调次数 = 本文件次数 - 声明 - 其它声明处的重复名字。
      const sameFileReferences = Math.max(0, occurrencesInOwnFile - 1);
      const otherFileReferences = Math.max(0, occurrencesInAllProduction - occurrencesInOwnFile);

      let category = null;
      if (otherFileReferences === 0 && sameFileReferences === 0 && occurrencesInTests === 0) {
        category = CATEGORY_ORPHAN;
      } else if (otherFileReferences === 0 && sameFileReferences === 0) {
        category = CATEGORY_TEST_ONLY;
      } else if (otherFileReferences === 0) {
        category = CATEGORY_SAME_FILE_ONLY;
      }
      if (category === null) continue;

      rows.push({
        category,
        kind: declaration.kind,
        name: declaration.name,
        location: `${relative(process.cwd(), file)}:${offsetToLine(raw, declaration.offset)}`,
        sameFileReferences,
        testReferences: occurrencesInTests,
      });
    }
  }

  const order = [CATEGORY_ORPHAN, CATEGORY_TEST_ONLY, CATEGORY_SAME_FILE_ONLY];
  rows.sort((left, right) => {
    const byCategory = order.indexOf(left.category) - order.indexOf(right.category);
    if (byCategory !== 0) return byCategory;
    return left.location.localeCompare(right.location);
  });

  const showAll = process.argv.includes("--all");
  const actionable = rows.filter(
    (row) => row.category === CATEGORY_ORPHAN || row.category === CATEGORY_TEST_ONLY,
  );
  const referenceOnly = rows.filter((row) => row.category === CATEGORY_SAME_FILE_ONLY);

  console.log("== 后端 pub 条目零生产引用清单（人工清单口径，非 cargo-udeps）==");
  console.log(
    `扫描 ${srcFiles.length} 个 src-tauri/src 文件、${testFiles.length} 个 src-tauri/tests 文件；` +
      `已剔除注释与字符串字面量；#[tauri::command] 不计入。`,
  );
  console.log(
    `候选合计 ${rows.length} 项：零引用 ${actionable.filter((r) => r.category === CATEGORY_ORPHAN).length}、` +
      `仅测试引用 ${actionable.filter((r) => r.category === CATEGORY_TEST_ONLY).length}、` +
      `仅本文件引用 ${referenceOnly.length}（参考项，默认不逐条列出，加 --all 查看）；` +
      `#[allow(dead_code)] 标记 ${allowDeadCodeCount} 处。`,
  );
  console.log("");

  if (actionable.length === 0) {
    console.log("零引用与仅测试引用两类均为空。");
  } else {
    console.log("分类 | 类型 | 名字 | 位置 | 本文件引用 | 测试引用");
    for (const row of actionable) {
      console.log(
        `${row.category} | ${row.kind} | ${row.name} | ${row.location} | ` +
          `${row.sameFileReferences} | ${row.testReferences}`,
      );
    }
    console.log("");
    console.log("「仅测试引用」是本项目最危险的一类：测试全绿会让人误以为它在生产上活着。");
  }

  if (showAll && referenceOnly.length > 0) {
    console.log("");
    console.log("== 参考项：仅本文件引用（pub 对 crate 外零使用者，多数只是多余的 pub）==");
    console.log("分类 | 类型 | 名字 | 位置 | 本文件引用 | 测试引用");
    for (const row of referenceOnly) {
      console.log(
        `${row.category} | ${row.kind} | ${row.name} | ${row.location} | ` +
          `${row.sameFileReferences} | ${row.testReferences}`,
      );
    }
  }
  return 0;
}

main().catch((error) => {
  console.error("死代码清单生成失败：", error);
  process.exit(1);
});
