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

use std::sync::Arc;

use thiserror::Error;

pub use crate::anyhow::Context;

/// Mailpot library error.
#[derive(Error, Debug)]
pub struct Error {
    kind: ErrorKind,
    source: Option<Arc<Self>>,
}

/// Mailpot library error.
#[derive(Error, Debug)]
pub enum ErrorKind {
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

    /// Error returned from an external user initiated operation such as
    /// deserialization or I/O.
    #[error(
        "Error returned from an external user initiated operation such as deserialization or I/O. \
         {0}"
    )]
    External(#[from] anyhow::Error),
    /// Generic
    #[error("{0}")]
    Generic(anyhow::Error),
    /// Error returned from sqlite3.
    #[error("Error returned from sqlite3 {0}.")]
    Sql(
        #[from]
        #[source]
        rusqlite::Error,
    ),
    /// Error returned from sqlite3.
    #[error("Error returned from sqlite3. {0}")]
    SqlLib(
        #[from]
        #[source]
        rusqlite::ffi::Error,
    ),
    /// Error returned from internal I/O operations.
    #[error("Error returned from internal I/O operations. {0}")]
    Io(#[from] ::std::io::Error),
    /// Error returned from e-mail protocol operations from `melib` crate.
    #[error("Error returned from e-mail protocol operations from `melib` crate. {0}")]
    Melib(#[from] melib::error::Error),
    /// Error from deserializing JSON values.
    #[error("Error from deserializing JSON values. {0}")]
    SerdeJson(#[from] serde_json::Error),
    /// Error returned from minijinja template engine.
    #[error("Error returned from minijinja template engine. {0}")]
    Template(#[from] minijinja::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{}", self.kind)
    }
}

impl From<ErrorKind> for Error {
    fn from(kind: ErrorKind) -> Self {
        Self { kind, source: None }
    }
}

macro_rules! impl_from {
    ($ty:ty) => {
        impl From<$ty> for Error {
            fn from(err: $ty) -> Self {
                Self {
                    kind: err.into(),
                    source: None,
                }
            }
        }
    };
}

impl_from! { anyhow::Error }
impl_from! { rusqlite::Error }
impl_from! { rusqlite::ffi::Error }
impl_from! { ::std::io::Error }
impl_from! { melib::error::Error }
impl_from! { serde_json::Error }
impl_from! { minijinja::Error }

impl Error {
    /// Helper function to create a new generic error message.
    pub fn new_external<S: Into<String>>(msg: S) -> Self {
        let msg = msg.into();
        ErrorKind::External(anyhow::Error::msg(msg)).into()
    }

    /// Chain an error by introducing a new head of the error chain.
    pub fn chain_err<E>(self, lambda: impl FnOnce() -> E) -> Self
    where
        E: Into<Self>,
    {
        let new_head: Self = lambda().into();
        Self {
            source: Some(Arc::new(self)),
            ..new_head
        }
    }

    /// Insert a source error into this Error.
    pub fn with_source<E>(self, source: E) -> Self
    where
        E: Into<Self>,
    {
        Self {
            source: Some(Arc::new(source.into())),
            ..self
        }
    }

    /// Getter for the kind field.
    pub fn kind(&self) -> &ErrorKind {
        &self.kind
    }

    /// Display error chain to user.
    pub fn display_chain(&'_ self) -> impl std::fmt::Display + '_ {
        ErrorChainDisplay {
            current: self,
            counter: 1,
        }
    }
}

impl From<String> for Error {
    fn from(s: String) -> Self {
        ErrorKind::Generic(anyhow::Error::msg(s)).into()
    }
}
impl From<&str> for Error {
    fn from(s: &str) -> Self {
        ErrorKind::Generic(anyhow::Error::msg(s.to_string())).into()
    }
}

/// Type alias for Mailpot library Results.
pub type Result<T> = std::result::Result<T, Error>;

struct ErrorChainDisplay<'e> {
    current: &'e Error,
    counter: usize,
}

impl std::fmt::Display for ErrorChainDisplay<'_> {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(ref source) = self.current.source {
            writeln!(fmt, "[{}] {}, caused by:", self.counter, self.current.kind)?;
            Self {
                current: source,
                counter: self.counter + 1,
            }
            .fmt(fmt)
        } else {
            writeln!(fmt, "[{}] {}", self.counter, self.current.kind)?;
            Ok(())
        }
    }
}
