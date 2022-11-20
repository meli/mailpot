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
    pub data_path: PathBuf,
}

impl Configuration {
    /*
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let db_path = db_path.into();
        Configuration {
            send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
            storage: "sqlite3".into(),
            data_path: db_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| db_path.clone()),
            db_path,
        }
    }
    */

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

    pub fn data_directory(&self) -> &Path {
        self.data_path.as_path()
    }

    pub fn db_path(&self) -> &Path {
        self.db_path.as_path()
    }

    pub fn default_path() -> Result<PathBuf> {
        let mut result =
            xdg::BaseDirectories::with_prefix("mailpot")?.place_config_file("config.toml")?;
        if result.starts_with("~") {
            result = Path::new(&std::env::var("HOME").context("No $HOME set.")?)
                .join(result.strip_prefix("~").context("Internal error while getting default database path: path starts with ~ but rust couldn't strip_refix(\"~\"")?);
        }
        Ok(result)
    }

    pub fn save_message_to_path(&self, msg: &str, mut path: PathBuf) -> Result<PathBuf> {
        if path.is_dir() {
            let now = Local::now().timestamp();
            path.push(format!("{}-failed.eml", now));
        }

        debug_assert!(path != self.db_path());
        let mut file = std::fs::File::create(&path)?;
        let metadata = file.metadata()?;
        let mut permissions = metadata.permissions();

        permissions.set_mode(0o600); // Read/write for owner only.
        file.set_permissions(permissions)?;
        file.write_all(msg.as_bytes())?;
        file.flush()?;
        Ok(path)
    }

    pub fn save_message(&self, msg: String) -> Result<PathBuf> {
        self.save_message_to_path(&msg, self.data_directory().to_path_buf())
    }
}

fn default_storage_fn() -> String {
    "sqlite3".to_string()
}
