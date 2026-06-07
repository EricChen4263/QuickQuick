#!/usr/bin/env python3
"""gen_ecdict_db.py 的回归测试（标准库 unittest，无需第三方依赖）。

聚焦两点：
1. build_db 从小 CSV 能写出非空 SQLite，且跳过空 translation 行（核心数据正确性）。
2. main 在 stdout 强制 utf-8 后，末尾中文日志能正常打印不抛 UnicodeEncodeError
   （复刻 Windows cp1252 崩溃场景：用 cp1252 编码的输出流跑 main 不应崩）。

用法：python3 -m unittest src-tauri/tools/test_gen_ecdict_db.py
（或在 tools 目录下 python3 -m unittest test_gen_ecdict_db）
"""

import csv
import importlib
import io
import sqlite3
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

import gen_ecdict_db  # noqa: E402


def _write_sample_csv(csv_path: Path) -> None:
    """写一份含 2 个有效词条 + 1 个空 translation 行的样例 CSV。"""
    with csv_path.open("w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(
            f, fieldnames=["word", "phonetic", "translation", "exchange"]
        )
        writer.writeheader()
        writer.writerow(
            {"word": "hello", "phonetic": "həˈləʊ",
             "translation": "你好", "exchange": ""}
        )
        writer.writerow(
            {"word": "world", "phonetic": "wɜːld",
             "translation": "世界", "exchange": "s:worlds"}
        )
        # 空 translation 行应被跳过（对本应用无展示价值）。
        writer.writerow(
            {"word": "noop", "phonetic": "", "translation": "", "exchange": ""}
        )


class BuildDbTest(unittest.TestCase):
    """build_db 数据正确性。"""

    def test_writes_nonempty_db_skipping_empty_translation(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            csv_path = Path(tmp) / "sample.csv"
            db_path = Path(tmp) / "out.db"
            _write_sample_csv(csv_path)

            written = gen_ecdict_db.build_db(csv_path, db_path)

            # 3 行输入，1 行空 translation 被跳过，应写 2 行。
            self.assertEqual(written, 2)
            self.assertTrue(db_path.exists() and db_path.stat().st_size > 0)

            conn = sqlite3.connect(db_path)
            try:
                rows = conn.execute(
                    "SELECT word, translation FROM ecdict ORDER BY word"
                ).fetchall()
            finally:
                conn.close()
            self.assertEqual(rows, [("hello", "你好"), ("world", "世界")])


class MainStdoutEncodingTest(unittest.TestCase):
    """main 末尾中文日志在 cp1252 流上不应崩（Windows 崩溃复刻）。"""

    def test_main_chinese_log_survives_cp1252_stdout(self) -> None:
        with tempfile.TemporaryDirectory() as tmp:
            csv_path = Path(tmp) / "sample.csv"
            db_path = Path(tmp) / "out.db"
            _write_sample_csv(csv_path)

            # 模拟 Windows 默认 stdout：底层 buffer 用 cp1252 编码。
            # 把 stdout 换成 cp1252 流后 reload 模块，触发脚本顶部的
            # reconfigure(utf-8)；未加该修复时，main 末尾打印中文会抛
            # UnicodeEncodeError（正是本测试要守卫的回归）。
            cp1252_stdout = io.TextIOWrapper(io.BytesIO(), encoding="cp1252")
            original_stdout = sys.stdout
            original_argv = sys.argv
            sys.stdout = cp1252_stdout
            sys.argv = ["gen_ecdict_db.py", str(csv_path), str(db_path)]
            try:
                module = importlib.reload(gen_ecdict_db)
                exit_code = module.main()
                sys.stdout.flush()
            finally:
                printed_bytes = cp1252_stdout.buffer.getvalue()
                sys.stdout = original_stdout
                sys.argv = original_argv
                importlib.reload(gen_ecdict_db)

            self.assertEqual(exit_code, 0)
            # 中文日志应以 utf-8 写入底层 buffer（含"已写入"字样）。
            self.assertIn("已写入".encode("utf-8"), printed_bytes)


if __name__ == "__main__":
    unittest.main()
