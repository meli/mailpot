/*
 * This file is part of mailpot
 *
 * Copyright 2020 - Manos Pitsidianakis
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of the
 * License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */

use std::{
    fs::{metadata, read_dir, OpenOptions},
    io,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

// Source: https://stackoverflow.com/a/64535181
fn is_output_file_outdated<P1, P2>(input: P1, output: P2) -> io::Result<bool>
where
    P1: AsRef<Path>,
    P2: AsRef<Path>,
{
    let out_meta = metadata(output);
    if let Ok(meta) = out_meta {
        let output_mtime = meta.modified()?;

        // if input file is more recent than our output, we are outdated
        let input_meta = metadata(input)?;
        let input_mtime = input_meta.modified()?;

        Ok(input_mtime > output_mtime)
    } else {
        // output file not found, we are outdated
        Ok(true)
    }
}

fn main() {
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=src/schema.sql.m4");

    if is_output_file_outdated("src/schema.sql.m4", "src/schema.sql").unwrap() {
        let output = Command::new("m4")
            .arg("./src/schema.sql.m4")
            .output()
            .unwrap();
        if String::from_utf8_lossy(&output.stdout).trim().is_empty() {
            panic!(
                "m4 output is empty. stderr was {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        let mut verify = Command::new("sqlite3")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        println!(
            "Verifying by creating an in-memory database in sqlite3 and feeding it the output \
             schema."
        );
        verify
            .stdin
            .take()
            .unwrap()
            .write_all(&output.stdout)
            .unwrap();
        let exit = verify.wait_with_output().unwrap();
        if !exit.status.success() {
            panic!(
                "sqlite3 could not read SQL schema: {}",
                String::from_utf8_lossy(&exit.stdout)
            );
        }
        let mut file = std::fs::File::create("./src/schema.sql").unwrap();
        file.write_all(&output.stdout).unwrap();
    }

    const MIGRATION_RS: &str = "src/migrations.rs.inc";

    let mut regen = false;
    let mut paths = vec![];
    let mut undo_paths = vec![];
    for entry in read_dir("migrations").unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() || path.extension().map(|os| os.to_str().unwrap()) != Some("sql") {
            continue;
        }
        if is_output_file_outdated(&path, MIGRATION_RS).unwrap() {
            regen = true;
        }
        if path
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .ends_with("undo.sql")
        {
            undo_paths.push(path);
        } else {
            paths.push(path);
        }
    }

    if regen {
        paths.sort();
        undo_paths.sort();
        let mut migr_rs = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(MIGRATION_RS)
            .unwrap();
        migr_rs
            .write_all(b"\n//(user_version, redo sql, undo sql\n&[")
            .unwrap();
        for (p, u) in paths.iter().zip(undo_paths.iter()) {
            // This should be a number string, padded with 2 zeros if it's less than 3
            // digits. e.g. 001, \d{3}
            let num = p.file_stem().unwrap().to_str().unwrap();
            if !u.file_name().unwrap().to_str().unwrap().starts_with(num) {
                panic!("Undo file {u:?} should match with {p:?}");
            }
            if num.parse::<u32>().is_err() {
                panic!("Migration file {p:?} should start with a number");
            }
            migr_rs.write_all(b"(").unwrap();
            migr_rs
                .write_all(num.trim_start_matches('0').as_bytes())
                .unwrap();
            migr_rs.write_all(b",\"").unwrap();

            migr_rs
                .write_all(std::fs::read_to_string(p).unwrap().as_bytes())
                .unwrap();
            migr_rs.write_all(b"\",\"").unwrap();
            migr_rs
                .write_all(std::fs::read_to_string(u).unwrap().as_bytes())
                .unwrap();
            migr_rs.write_all(b"\"),").unwrap();
        }
        migr_rs.write_all(b"]").unwrap();
        migr_rs.flush().unwrap();
    }
}
