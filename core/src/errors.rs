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

pub use crate::anyhow::Context;
pub use error_chain::ChainedError;

// Create the Error, ErrorKind, ResultExt, and Result types
error_chain! {
   errors {
       PostRejected(reason: String) {
           description("Post rejected")
           display("Your post has been rejected: {}", reason)
       }

       NotFound(model: &'static str) {
           description("Not found")
           display("This {} is not present in the database.", model)
       }

       InvalidRequest(reason: String) {
           description("List request is invalid")
           display("Your list request has been found invalid: {}.", reason)
       }

       Information(reason: String) {
           description("")
           display("{}.", reason)
       }
   }
   foreign_links {
       Logic(anyhow::Error);
       Sql(rusqlite::Error);
       Io(::std::io::Error);
       Xdg(xdg::BaseDirectoriesError);
       Melib(melib::error::Error);
       Configuration(toml::de::Error);
       SerdeJson(serde_json::Error);
   }
}
