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
    io::{Read, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use chrono::prelude::*;

use super::errors::*;

/// How to send e-mail.
#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(tag = "type", content = "value")]
pub enum SendMail {
    /// A `melib` configuration for talking to an SMTP server.
    Smtp(melib::smtp::SmtpServerConf),
    /// A plain shell command passed to `sh -c` with the e-mail passed in the
    /// stdin.
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
    /// Instance administrators (List of e-mail addresses). Optional.
    #[serde(default)]
    pub administrators: Vec<String>,
}

impl Configuration {
    /// Create a new configuration value from a given database path value.
    ///
    /// If you wish to create a new database with this configuration, use
    /// [`Connection::open_or_create_db`](crate::Connection::open_or_create_db).
    /// To open an existing database, use
    /// [`Database::open_db`](crate::Connection::open_db).
    pub fn new(db_path: impl Into<PathBuf>) -> Self {
        let db_path = db_path.into();
        Self {
            send_mail: SendMail::ShellCommand("/usr/bin/false".to_string()),
            data_path: db_path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| db_path.clone()),
            administrators: vec![],
            db_path,
        }
    }

    /// Deserialize configuration from TOML file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path = path.as_ref();
        let mut s = String::new();
        let mut file = std::fs::File::open(path)
            .with_context(|| format!("Configuration file {} not found.", path.display()))?;
        file.read_to_string(&mut s)
            .with_context(|| format!("Could not read from file {}.", path.display()))?;
        let config: Self = toml::from_str(&s)
            .map_err(anyhow::Error::from)
            .with_context(|| {
                format!(
                    "Could not parse configuration file `{}` successfully: ",
                    path.display()
                )
            })?;

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
        let mut file = std::fs::File::create(&path)
            .with_context(|| format!("Could not create file {}.", path.display()))?;
        let metadata = file
            .metadata()
            .with_context(|| format!("Could not fstat file {}.", path.display()))?;
        let mut permissions = metadata.permissions();

        permissions.set_mode(0o600); // Read/write for owner only.
        file.set_permissions(permissions)
            .with_context(|| format!("Could not chmod 600 file {}.", path.display()))?;
        file.write_all(msg.as_bytes())
            .with_context(|| format!("Could not write message to file {}.", path.display()))?;
        file.flush()
            .with_context(|| format!("Could not flush message I/O to file {}.", path.display()))?;
        Ok(path)
    }

    /// Save message to the data directory.
    pub fn save_message(&self, msg: String) -> Result<PathBuf> {
        self.save_message_to_path(&msg, self.data_directory().to_path_buf())
    }

    /// Serialize configuration to a TOML string.
    pub fn to_toml(&self) -> String {
        toml::ser::to_string(self).expect("Could not serialize config to TOML")
    }
}

#[cfg(test)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    #[test]
    fn test_config_parse_error() {
        let tmp_dir = TempDir::new().unwrap();
        let conf_path = tmp_dir.path().join("conf.toml");
        std::fs::write(&conf_path, b"afjsad skas as a as\n\n\n\n\t\x11\n").unwrap();

        assert_eq!(
            Configuration::from_file(&conf_path)
                .unwrap_err()
                .display_chain()
                .to_string(),
            format!(
                "[1] Could not parse configuration file `{}` successfully:  Caused by:\n[2] Error: TOML parse error at line 1, column 8\n  |\n1 | afjsad skas as a as\n  |        ^\nexpected `.`, `=`\n\n",
                conf_path.display()
            ),
        );
    }
}
