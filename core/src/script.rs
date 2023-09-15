/*
 * This file is part of mailpot
 *
 * Copyright 2023 - Manos Pitsidianakis
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

//! fixme

use rhai::{Engine, EvalAltResult};

/// fixme
#[derive(Debug)]
pub struct Error {
    engine: ScriptEngine,
    val: Box<dyn std::error::Error + Send>,
    kind: ErrorKind,
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

/// fixme
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum ErrorKind {
    /// fixme
    Parsing,
    /// fixme
    System,
    #[default]
    /// fixme
    Other,
}

impl std::error::Error for Error {}

/// fixme
#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub enum ScriptEngine {
    #[default]
    /// fixme
    Rhai,
    /// fixme
    POSIXShell,
}

impl From<Box<EvalAltResult>> for Error {
    fn from(val: Box<EvalAltResult>) -> Self {
        Self {
            engine: ScriptEngine::Rhai,
            val,
            kind: ErrorKind::Other,
        }
    }
}

/// fixme
pub fn validate(script: &str, engine_kind: ScriptEngine) -> Result<(), Error> {
    let engine = Engine::new();
    engine.eval(script)?;
    Ok(())
}
