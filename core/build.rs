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
    fs::OpenOptions,
    process::{Command, Stdio},
};

// // Source: https://stackoverflow.com/a/64535181
// fn is_output_file_outdated<P1, P2>(input: P1, output: P2) -> io::Result<bool>
// where
//     P1: AsRef<Path>,
//     P2: AsRef<Path>,
// {
//     let out_meta = metadata(output);
//     if let Ok(meta) = out_meta {
//         let output_mtime = meta.modified()?;
//
//         // if input file is more recent than our output, we are outdated
//         let input_meta = metadata(input)?;
//         let input_mtime = input_meta.modified()?;
//
//         Ok(input_mtime > output_mtime)
//     } else {
//         // output file not found, we are outdated
//         Ok(true)
//     }
// }

include!("make_migrations.rs");

const MIGRATION_RS: &str = "src/migrations.rs.inc";

fn main() {
    println!("cargo:rerun-if-changed=src/migrations.rs.inc");
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=src/schema.sql.m4");

    let mut output = Command::new("m4")
        .arg("./src/schema.sql.m4")
        .output()
        .unwrap();
    if String::from_utf8_lossy(&output.stdout).trim().is_empty() {
        panic!(
            "m4 output is empty. stderr was {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    make_migrations("migrations", MIGRATION_RS, &mut output.stdout);
    let mut verify = Command::new(std::env::var("SQLITE_BIN").unwrap_or("sqlite3".into()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    println!(
        "Verifying by creating an in-memory database in sqlite3 and feeding it the output schema."
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
