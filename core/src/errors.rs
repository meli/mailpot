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

//! Errors of this library.

pub use crate::anyhow::Context;

use thiserror::Error;

/// Mailpot library error.
#[derive(Error, Debug)]
pub enum Error {
    /// Post rejected.
    #[error("Your post has been rejected: {0}")]
    PostRejected(String),
    /// An entry was not found in the database.
    #[error("This {0} is not present in the database.")]
    NotFound(&'static str),
    /// A request was invalid.
    #[error("Your list request has been found invalid: {0}.")]
    InvalidRequest(String),
    /// An error happened and it was handled internally.
    #[error("An error happened and it was handled internally: {0}.")]
    Information(String),
    /// An error that shouldn't happen and should be reported.
    #[error("An error that shouldn't happen and should be reported: {0}.")]
    Bug(String),

    /// Error returned from an external user initiated operation such as deserialization or I/O.
    #[error(
        "Error returned from an external user initiated operation such as deserialization or I/O."
    )]
    External(#[from] anyhow::Error),
    /// Generic
    #[error("Error: {0}")]
    Generic(anyhow::Error),
    /// Error returned from sqlite3.
    #[error("Error returned from sqlite3.")]
    Sql(#[from] rusqlite::Error),
    /// Error returned from internal I/O operations.
    #[error("Error returned from internal I/O operations.")]
    Io(#[from] ::std::io::Error),
    /// Error returned from e-mail protocol operations from `melib` crate.
    #[error("Error returned from e-mail protocol operations from `melib` crate.")]
    Melib(#[from] melib::error::Error),
    /// Error from deserializing JSON values.
    #[error("Error from deserializing JSON values.")]
    SerdeJson(#[from] serde_json::Error),
    /// Error returned from minijinja template engine.
    #[error("Error returned from minijinja template engine.")]
    Template(#[from] minijinja::Error),
}

impl Error {
    /// Helper function to create a new generic error message.
    pub fn new_external<S: Into<String>>(msg: S) -> Self {
        let msg = msg.into();
        Self::External(anyhow::Error::msg(msg))
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        Self::Generic(anyhow::Error::msg(s))
    }
}
impl From<&str> for Error {
    fn from(s: &str) -> Self {
        Self::Generic(anyhow::Error::msg(s.to_string()))
    }
}

/// Type alias for Mailpot library Results.
pub type Result<T> = std::result::Result<T, Error>;
