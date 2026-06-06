#!/usr/bin/env python3
"""把 ECDICT 源 CSV 生成精简版本地 SQLite 词典库。

仅保留运行时词典展示所需的 4 列（word / phonetic / translation / exchange），
丢弃 definition / pos / collins / oxford / tag / bnc / frq / detail / audio 等，
将 ~110MB 的全量库压到可随应用打包的体积。

用法:
    python tools/gen_ecdict_db.py <ecdict.csv> <out.db>
    # 省略参数时默认读 src-tauri 同级的 ecdict.csv，写 resources/ecdict.db

设计要点:
- 大小写不敏感查询走 `LOWER(word)`，故建 `LOWER(word)` 表达式索引（而非新增小写列），
  省一列存储且与 DAO 的 `WHERE LOWER(word)=LOWER(?)` 直接对应、命中索引。
- 可复跑：每次重建表（DROP IF EXISTS），跑完打印写入行数，便于 CI 校验非空。
- 跳过 translation 为空的行（无中文释义的词条对本应用无展示价值）。
"""

import csv
import sqlite3
import sys
from pathlib import Path

# CSV 列名（与 skywind3000/ECDICT ecdict.csv 表头一致），只取这 4 个。
KEPT_COLUMNS = ("word", "phonetic", "translation", "exchange")

# CSV 单字段可能超出 Python 默认上限，放宽以免长 detail 行触发 field larger than limit。
csv.field_size_limit(10 * 1024 * 1024)


def build_db(csv_path: Path, db_path: Path) -> int:
    """读取 CSV，写入精简 SQLite，返回写入行数。"""
    db_path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(db_path)
    try:
        cur = conn.cursor()
        cur.execute("DROP TABLE IF EXISTS ecdict")
        cur.execute(
            "CREATE TABLE ecdict ("
            "word TEXT NOT NULL, "
            "phonetic TEXT, "
            "translation TEXT, "
            "exchange TEXT)"
        )

        rows = _read_rows(csv_path)
        cur.executemany(
            "INSERT INTO ecdict (word, phonetic, translation, exchange) "
            "VALUES (?, ?, ?, ?)",
            rows,
        )

        # LOWER(word) 表达式索引：支撑 DAO 大小写不敏感查询命中索引。
        cur.execute(
            "CREATE INDEX idx_ecdict_lower_word ON ecdict (LOWER(word))"
        )
        conn.commit()
        return _count(cur)
    finally:
        conn.close()


def _read_rows(csv_path: Path):
    """逐行产出 (word, phonetic, translation, exchange)；跳过空 translation。"""
    result = []
    with csv_path.open(newline="", encoding="utf-8") as f:
        reader = csv.DictReader(f)
        for record in reader:
            translation = (record.get("translation") or "").strip()
            if not translation:
                continue
            result.append(
                tuple((record.get(col) or "").strip() for col in KEPT_COLUMNS)
            )
    return result


def _count(cur: sqlite3.Cursor) -> int:
    """统计已写入行数（executemany 后 rowcount 在部分驱动不可靠）。"""
    cur.execute("SELECT COUNT(*) FROM ecdict")
    return int(cur.fetchone()[0])


def main() -> int:
    """命令行入口：解析参数、生成库、打印行数。"""
    base = Path(__file__).resolve().parent.parent  # src-tauri/
    csv_path = Path(sys.argv[1]) if len(sys.argv) > 1 else base / "ecdict.csv"
    db_path = Path(sys.argv[2]) if len(sys.argv) > 2 else base / "resources" / "ecdict.db"

    if not csv_path.exists():
        print(f"[gen_ecdict_db] 源 CSV 不存在: {csv_path}", file=sys.stderr)
        return 1

    written = build_db(csv_path, db_path)
    print(f"[gen_ecdict_db] 已写入 {written} 行 -> {db_path}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
