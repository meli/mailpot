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
    println!("cargo:rerun-if-changed=../docs/command.mdoc");
    println!("cargo:rerun-if-changed=../docs/list.mdoc");
    println!("cargo:rerun-if-changed=../docs/error_queue.mdoc");
    println!("cargo:rerun-if-changed=../docs/main.mdoc");
    println!("cargo:rerun-if-changed=../docs/header.mdoc");
    println!("cargo:rerun-if-changed=../docs/footer.mdoc");
    println!("cargo:rerun-if-changed=../docs/mailpot.1.m4");
    println!("cargo:rerun-if-changed=../docs/mailpot.1");
    println!("cargo:rerun-if-changed=../docs");
    println!("cargo:rerun-if-changed=./src/main.rs");
    println!("build running");
    std::env::set_current_dir("..").expect("could not chdir('..')");

    let output = Command::new("m4")
        .arg("./docs/mailpot.1.m4")
        .output()
        .unwrap();
    let mut file = std::fs::File::create("./docs/mailpot.1").unwrap();
    file.write_all(&output.stdout).unwrap();
}
