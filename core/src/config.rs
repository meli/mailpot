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
use std::cell::{Cell, RefCell};
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

thread_local!(pub static CONFIG: RefCell<Configuration> = RefCell::new(Configuration::new()));
thread_local!(static CONFIG_INIT: Cell<bool> = Cell::new(false));

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
    pub db_path: PathBuf,
}

impl Default for Configuration {
    fn default() -> Self {
        Self::new()
    }
}

impl Configuration {
    pub(crate) fn new() -> Self {
        Configuration {
            send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
            storage: "sqlite3".into(),
            db_path: ".".into(),
        }
    }

    pub fn init_with(self) -> Result<()> {
        CONFIG.with(|f| {
            *f.borrow_mut() = self;
        });
        CONFIG_INIT.with(|f| {
            f.set(true);
        });

        Ok(())
    }

    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let mut s = String::new();
        let mut file = std::fs::File::open(path)?;
        file.read_to_string(&mut s)?;
        let config: Configuration = toml::from_str(&s).context(format!(
            "Could not parse configuration file `{}` succesfully: ",
            path.display()
        ))?;

        Ok(config)
    }

    pub fn init() -> Result<()> {
        if CONFIG_INIT.with(|f| f.get()) {
            return Ok(());
        }
        let mut path =
            xdg::BaseDirectories::with_prefix("mailpot")?.place_config_file("config.toml")?;
        if !path.exists() {
            return Err(format!("Configuration file {} doesn't exist", path.display()).into());
        }
        if path.starts_with("~") {
            path = Path::new(&std::env::var("HOME").context("No $HOME set.")?)
                .join(path.strip_prefix("~").context("Internal error while getting default database path: path starts with ~ but rust couldn't strip_refix(\"~\"")?)
                .into();
        }
        let config: Configuration = Self::from_file(&path)?;
        config.init_with()
    }

    pub fn data_directory() -> Result<PathBuf> {
        Ok(xdg::BaseDirectories::with_prefix("mailpot")?
            .get_data_home()
            .canonicalize()?)
    }

    pub fn default_path() -> Result<PathBuf> {
        let mut result =
            xdg::BaseDirectories::with_prefix("mailpot")?.place_config_file("config.toml")?;
        if result.starts_with("~") {
            result = Path::new(&std::env::var("HOME").context("No $HOME set.")?)
                .join(result.strip_prefix("~").context("Internal error while getting default database path: path starts with ~ but rust couldn't strip_refix(\"~\"")?)
                .into();
        }
        Ok(result)
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
