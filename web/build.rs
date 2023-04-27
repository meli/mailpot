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

fn commit_sha() {
    build_info_build::build_script();

    if let Ok(s) = std::fs::read_to_string(".cargo_vcs_info.json") {
        const KEY: &str = "\"sha1\":";

        fn find_tail<'str>(str: &'str str, tok: &str) -> Option<&'str str> {
            let i = str.find(tok)?;
            Some(&str[(i + tok.len())..])
        }

        if let Some(mut tail) = find_tail(&s, KEY) {
            while !tail.starts_with('"') && !tail.is_empty() {
                tail = &tail[1..];
            }
            if !tail.is_empty() {
                // skip "
                tail = &tail[1..];
                if let Some(end) = find_tail(tail, "\"") {
                    let end = tail.len() - end.len() - 1;
                    println!("cargo:rustc-env=PACKAGE_GIT_SHA={}", &tail[..end]);
                }
            }
        }
    }
}

#[cfg(feature = "zstd")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Embed HTML templates as zstd compressed byte slices into binary.
    // [tag:embed_templates]

    use std::{
        fs::{create_dir_all, read_dir, OpenOptions},
        io::{Read, Write},
        path::PathBuf,
    };
    create_dir_all("./src/minijinja_utils")?;
    let mut compressed = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open("./src/minijinja_utils/compressed.data")?;

    println!("cargo:rerun-if-changed=./src/templates");
    println!("cargo:rerun-if-changed=./src/minijinja_utils/compressed.rs");

    let mut templates: Vec<(String, PathBuf)> = vec![];
    let root_prefix: PathBuf = "./src/templates/".into();
    let mut dirs: Vec<PathBuf> = vec!["./src/templates/".into()];
    while let Some(dir) = dirs.pop() {
        for entry in read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                dirs.push(path);
            } else if path.extension().map(|s| s == "html").unwrap_or(false) {
                templates.push((path.strip_prefix(&root_prefix)?.display().to_string(), path));
            }
        }
    }

    compressed.write_all(b"&[")?;
    for (name, template_path) in templates {
        let mut templ = OpenOptions::new()
            .write(false)
            .create(false)
            .read(true)
            .open(&template_path)?;
        let mut templ_bytes = vec![];
        let mut compressed_bytes = vec![];
        let mut enc = zstd::stream::write::Encoder::new(&mut compressed_bytes, 21)?;
        enc.include_checksum(true)?;
        templ.read_to_end(&mut templ_bytes)?;
        enc.write_all(&templ_bytes)?;
        enc.finish()?;
        compressed.write_all(b"(\"")?;
        compressed.write_all(name.as_bytes())?;
        compressed.write_all(b"\",&")?;
        compressed.write_all(format!("{:?}", compressed_bytes).as_bytes())?;
        compressed.write_all(b"),")?;
    }
    compressed.write_all(b"]")?;

    commit_sha();
    Ok(())
}

#[cfg(not(feature = "zstd"))]
fn main() {
    commit_sha();
}
