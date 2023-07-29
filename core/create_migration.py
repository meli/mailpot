import json
from pathlib import Path
import re
import sys
import pprint
import argparse


def make_undo(id: str) -> str:
    return f"DELETE FROM settings_json_schema WHERE id = '{id}';"


def make_redo(id: str, value: str) -> str:
    return f"""INSERT OR REPLACE INTO settings_json_schema(id, value) VALUES('{id}', '{value}');"""


class Migration:
    patt = re.compile(r"(\d+)[.].*sql")

    def __init__(self, path: Path):
        name = path.name
        self.path = path
        self.is_data = "data" in name
        self.is_undo = "undo" in name
        m = self.patt.match(name)
        self.seq = int(m.group(1))
        self.name = name

    def __str__(self) -> str:
        return str(self.seq)

    def __repr__(self) -> str:
        return f"Migration(seq={self.seq},name={self.name},path={self.path},is_data={self.is_data},is_undo={self.is_undo})"


if __name__ == "__main__":
    parser = argparse.ArgumentParser(
        prog="Create migrations", description="", epilog=""
    )
    parser.add_argument("--data", action="store_true")
    parser.add_argument("--settings", action="store_true")
    parser.add_argument("--name", type=str, default=None)
    parser.add_argument("--dry-run", action="store_true")
    args = parser.parse_args()
    migrations = {}
    last = -1
    for f in Path(".").glob("migrations/*.sql"):
        m = Migration(f)
        last = max(last, m.seq)
        seq = str(m)
        if seq not in migrations:
            if m.is_undo:
                migrations[seq] = (None, m)
            else:
                migrations[seq] = (m, None)
        else:
            if m.is_undo:
                redo, _ = migrations[seq]
                migrations[seq] = (redo, m)
            else:
                _, undo = migrations[seq]
                migrations[seq] = (m, undo)
    # pprint.pprint(migrations)
    if args.data:
        data = ".data"
    else:
        data = ""
    new_name = f"{last+1:0>3}{data}.sql"
    new_undo_name = f"{last+1:0>3}{data}.undo.sql"
    if not args.dry_run:
        redo = ""
        undo = ""
        if args.settings:
            if not args.name:
                print("Please define a --name.")
                sys.exit(1)
            redo = make_redo(args.name, "{}")
            undo = make_undo(args.name)
            name = args.name.lower() + ".json"
            with open(Path("settings_json_schemas") / name, "x") as file:
                file.write("{}")
        with open(Path("migrations") / new_name, "x") as file, open(
            Path("migrations") / new_undo_name, "x"
        ) as undo_file:
            file.write(redo)
            undo_file.write(undo)
    print(f"Created to {new_name} and {new_undo_name}.")
