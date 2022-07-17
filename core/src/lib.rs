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
// `error_chain!` can recurse deeply
#![recursion_limit = "1024"]
//#![warn(missing_docs)]

use log::{info, trace};
#[macro_use]
extern crate error_chain;
extern crate anyhow;
#[macro_use]
pub extern crate serde;

pub use melib;
pub use serde_json;

pub mod config;
pub mod mail;
pub mod models;
use models::*;
pub mod errors;
use errors::*;
pub mod db;

pub use config::{Configuration, SendMail};
pub use db::Database;
pub use errors::*;
