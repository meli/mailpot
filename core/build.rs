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

use std::io::Write;
use std::process::{Command, Stdio};

fn main() {
    println!("cargo:rerun-if-changed=src/schema.sql.m4");

    let output = Command::new("m4")
        .arg("./src/schema.sql.m4")
        .output()
        .unwrap();
    let mut verify = Command::new("sqlite3")
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
