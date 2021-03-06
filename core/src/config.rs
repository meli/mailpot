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

use super::errors::*;
use chrono::prelude::*;
use std::cell::RefCell;
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

thread_local!(pub static CONFIG: RefCell<Configuration> = RefCell::new(Configuration::new()));

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "value")]
pub enum SendMail {
    Smtp(melib::smtp::SmtpServerConf),
    ShellCommand(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Configuration {
    pub send_mail: SendMail,
    #[serde(default = "default_storage_fn")]
    pub storage: String,
}

impl Configuration {
    pub(crate) fn new() -> Self {
        Configuration {
            send_mail: SendMail::ShellCommand(String::new()),
            storage: "sqlite3".into(),
        }
    }

    pub fn init() -> Result<()> {
        let path =
            xdg::BaseDirectories::with_prefix("mailpot")?.place_config_file("config.toml")?;
        let mut s = String::new();
        let mut file = std::fs::File::open(&path)?;
        file.read_to_string(&mut s)?;
        let config: Configuration = toml::from_str(&s)?;
        CONFIG.with(|f| {
            *f.borrow_mut() = config;
        });

        Ok(())
    }

    pub fn data_directory() -> Result<PathBuf> {
        Ok(xdg::BaseDirectories::with_prefix("mailpot")?.get_data_home())
    }

    pub fn save_message_to_path(msg: &str, mut path: PathBuf) -> Result<PathBuf> {
        let now = Local::now().timestamp();
        path.push(format!("{}-failed.eml", now));

        let mut file = std::fs::File::create(&path)?;
        let metadata = file.metadata()?;
        let mut permissions = metadata.permissions();

        permissions.set_mode(0o600); // Read/write for owner only.
        file.set_permissions(permissions)?;
        file.write_all(msg.as_bytes())?;
        file.flush()?;
        Ok(path)
    }

    pub fn save_message(msg: String) -> Result<PathBuf> {
        match Configuration::data_directory()
            .and_then(|path| Self::save_message_to_path(&msg, path))
        {
            Ok(p) => return Ok(p),
            Err(err) => {
                eprintln!("{}", err);
            }
        };
        match Self::save_message_to_path(&msg, PathBuf::from(".")) {
            Ok(p) => return Ok(p),
            Err(err) => {
                eprintln!("{}", err);
            }
        }
        let temp_path = std::env::temp_dir();
        Self::save_message_to_path(&msg, temp_path)
    }
}

fn default_storage_fn() -> String {
    "sqlite3".to_string()
}
