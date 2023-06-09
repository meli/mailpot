/*
 * This file is part of mailpot
 *
 * Copyright 2023 - Manos Pitsidianakis
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

use std::{fs::read_dir, io::Write, path::Path};

/// Scans migrations directory for file entries, and creates a rust file with an array containing
/// the migration slices.
///
///
/// If a migration is a data migration (not a CREATE, DROP or ALTER statement) it is appended to
/// the schema file.
pub fn make_migrations<M: AsRef<Path>, O: AsRef<Path>>(
    migrations_path: M,
    output_file: O,
    schema_file: &mut Vec<u8>,
) {
    let migrations_folder_path = migrations_path.as_ref();
    let output_file_path = output_file.as_ref();

    let mut regen = false;
    let mut paths = vec![];
    let mut undo_paths = vec![];
    for entry in read_dir(migrations_folder_path).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        if path.is_dir() || path.extension().map(|os| os.to_str().unwrap()) != Some("sql") {
            continue;
        }
        if is_output_file_outdated(&path, output_file_path).unwrap() {
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
            .open(output_file_path)
            .unwrap();
        migr_rs
            .write_all(b"\n//(user_version, redo sql, undo sql\n&[")
            .unwrap();
        for (i, (p, u)) in paths.iter().zip(undo_paths.iter()).enumerate() {
            // This should be a number string, padded with 2 zeros if it's less than 3
            // digits. e.g. 001, \d{3}
            let mut num = p.file_stem().unwrap().to_str().unwrap();
            let is_data = num.ends_with(".data");
            if is_data {
                num = num.strip_suffix(".data").unwrap();
            }

            if !u.file_name().unwrap().to_str().unwrap().starts_with(num) {
                panic!("Undo file {u:?} should match with {p:?}");
            }

            if num.parse::<u32>().is_err() {
                panic!("Migration file {p:?} should start with a number");
            }
            assert_eq!(num.parse::<usize>().unwrap(), i + 1, "migration sql files should start with 1, not zero, and no intermediate numbers should be missing. Panicked on file: {}", p.display());
            migr_rs.write_all(b"(").unwrap();
            migr_rs
                .write_all(num.trim_start_matches('0').as_bytes())
                .unwrap();
            migr_rs.write_all(b",r###\"").unwrap();

            let redo = std::fs::read_to_string(p).unwrap();
            migr_rs.write_all(redo.trim().as_bytes()).unwrap();
            migr_rs.write_all(b"\"###,r###\"").unwrap();
            migr_rs
                .write_all(std::fs::read_to_string(u).unwrap().trim().as_bytes())
                .unwrap();
            migr_rs.write_all(b"\"###),").unwrap();
            if is_data {
                schema_file.extend(b"\n\n-- ".iter());
                schema_file.extend(num.as_bytes().iter());
                schema_file.extend(b".data.sql\n\n".iter());
                schema_file.extend(redo.into_bytes().into_iter());
            }
        }
        migr_rs.write_all(b"]").unwrap();
        migr_rs.flush().unwrap();
    }
}
