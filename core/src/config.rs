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
use std::io::{Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// How to send e-mail.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "value")]
pub enum SendMail {
    /// A `melib` configuration for talking to an SMTP server.
    Smtp(melib::smtp::SmtpServerConf),
    /// A plain shell command passed to `sh -c` with the e-mail passed in the stdin.
    ShellCommand(String),
}

/// The configuration for the mailpot database and the mail server.
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Configuration {
    /// How to send e-mail.
    pub send_mail: SendMail,
    /// The location of the sqlite3 file.
    pub db_path: PathBuf,
    /// The directory where data are stored.
    pub data_path: PathBuf,
}

impl Configuration {
    /// Create a new configuration value from a given database path value.
    ///
    /// If you wish to create a new database with this configuration, use [`Connection::open_or_create_db`](crate::Connection::open_or_create_db).
    /// To open an existing database, use [`Database::open_db`](crate::Connection::open_db).
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let db_path = db_path.into();
        Self {
            send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
            data_path: db_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| db_path.clone()),
            db_path,
        }
    }

    /// Deserialize configuration from TOML file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let mut s = String::new();
        let mut file = std::fs::File::open(path)?;
        file.read_to_string(&mut s)?;
        let config: Self = toml::from_str(&s).context(format!(
            "Could not parse configuration file `{}` succesfully: ",
            path.display()
        ))?;

        Ok(config)
    }

    /// The saved data path.
    pub fn data_directory(&self) -> &Path {
        self.data_path.as_path()
    }

    /// The sqlite3 database path.
    pub fn db_path(&self) -> &Path {
        self.db_path.as_path()
    }

    /// Save message to a custom path.
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

    /// Save message to the data directory.
    pub fn save_message(&self, msg: String) -> Result<PathBuf> {
        self.save_message_to_path(&msg, self.data_directory().to_path_buf())
    }

    /// Serialize configuration to a TOML string.
    pub fn to_toml(&self) -> String {
        toml::Value::try_from(self)
            .expect("Could not serialize config to TOML")
            .to_string()
    }
}
