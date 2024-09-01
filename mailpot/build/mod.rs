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
    fs::File,
    io::{prelude::*, Write},
    path::Path,
    process::{Command, Stdio},
};

use jsonschema::JSONSchema;
use quote::{format_ident, quote};
use serde_json::Value;

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
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=src/schema.sql.m4");
    println!("cargo:rerun-if-changed=settings_json_schemas");

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
    let user_version: i32 = make_migrations("migrations", MIGRATION_RS, &mut output.stdout);
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
    file.write_all(
        format!("\n\n-- Set current schema version.\n\nPRAGMA user_version = {user_version};\n")
            .as_bytes(),
    )
    .unwrap();

    make_mod_message_filter_settings().unwrap();
}

fn make_mod_message_filter_settings() -> std::io::Result<()> {
    let mut output_file = File::create("../mailpot-cli/src/message_filter_settings.rs")
        .expect("Unable to open output file");
    let mut output_string = r##"// @generated
//
// This file is part of mailpot
//
// Copyright 2023 - Manos Pitsidianakis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use clap::builder::TypedValueParser;

"##
    .to_string();

    let mut names = vec![];
    let mut str_names = vec![];
    for entry in Path::new("./settings_json_schemas/").read_dir()?.flatten() {
        println!("cargo:rerun-if-changed={}", entry.path().display());
        let mut file = File::open(entry.path()).unwrap_or_else(|err| {
            panic!("Unable to open file `{}` {}", entry.path().display(), err)
        });

        let mut src = String::new();
        file.read_to_string(&mut src).expect("Unable to read file");

        let schema: Value = serde_json::from_str(&src).unwrap();
        let _compiled = JSONSchema::compile(&schema).expect("A valid schema");
        let name = schema["$defs"]
            .as_object()
            .expect("$defs not a json object")
            .keys()
            .next()
            .expect("#defs is an empty object")
            .as_str()
            .to_string();
        names.push(format_ident!("{}", name));
        str_names.push(name);
    }
    let literal_enum = quote! {
        #[allow(clippy::enum_variant_names)]
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum MessageFilterSettingName {
            #(#names),*
        }

        impl ::std::str::FromStr for MessageFilterSettingName {
            type Err = String;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                #![allow(clippy::suspicious_else_formatting)]

                #(if s.eq_ignore_ascii_case(stringify!(#names)) {
                    return Ok(Self::#names);
                }
                )*

                Err(format!("Unrecognized value: {s}"))
            }
        }
        impl ::std::fmt::Display for MessageFilterSettingName {
            fn fmt(&self, fmt: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
                write!(fmt, "{}",
                    match self {
                        #(Self::#names => stringify!(#names)),*
                    }
                )
            }
        }
    };
    output_string.push_str(&literal_enum.to_string());

    let value_parser = quote! {
        #[derive(Clone, Copy, Debug)]
        pub struct MessageFilterSettingNameValueParser;

        impl MessageFilterSettingNameValueParser {
            pub fn new() -> Self {
                Self
            }
        }

        impl TypedValueParser for MessageFilterSettingNameValueParser {
            type Value = MessageFilterSettingName;

            fn parse_ref(
                &self,
                cmd: &clap::Command,
                arg: Option<&clap::Arg>,
                value: &std::ffi::OsStr,
            ) -> std::result::Result<Self::Value, clap::Error> {
                TypedValueParser::parse(self, cmd, arg, value.to_owned())
            }

            fn parse(
                &self,
                cmd: &clap::Command,
                _arg: Option<&clap::Arg>,
                value: std::ffi::OsString,
            ) -> std::result::Result<Self::Value, clap::Error> {
                use std::str::FromStr;

                use clap::error::ErrorKind;

                if value.is_empty() {
                    return Err(cmd.clone().error(
                            ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand,
                            "Message filter setting name value required",
                    ));
                }
                Self::Value::from_str(value.to_str().ok_or_else(|| {
                    cmd.clone().error(
                        ErrorKind::InvalidValue,
                        "Message filter setting name value is not an UTF-8 string",
                    )
                })?)
                .map_err(|err| cmd.clone().error(ErrorKind::InvalidValue, err))
            }

            fn possible_values(&self) -> Option<Box<dyn Iterator<Item = clap::builder::PossibleValue>>> {
                Some(Box::new(
                        [#(#str_names),*]
                        .iter()
                        .map(clap::builder::PossibleValue::new),
                ))
            }
        }

        impl Default for MessageFilterSettingNameValueParser {
            fn default() -> Self {
                Self::new()
            }
        }
    };
    output_string.push_str(&value_parser.to_string());

    output_file.write_all(output_string.as_bytes()).unwrap();
    output_file.write_all("\n".as_bytes()).unwrap();
    Ok(())
}
