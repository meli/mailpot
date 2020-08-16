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
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/schema.sql.m4");

    let output = Command::new("m4")
        .arg("./src/schema.sql.m4")
        .output()
        .unwrap();
    let mut file = std::fs::File::create("./src/schema.sql").unwrap();
    file.write_all(&output.stdout).unwrap();
}
