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

// Create the Error, ErrorKind, ResultExt, and Result types
error_chain! {
   errors {
       PostRejected(reason: String) {
           description("Post rejected")
           display("Your post has been rejected: {}", reason)
       }
   }
   foreign_links {
       Sql(rusqlite::Error);
       Io(::std::io::Error);
       Xdg(xdg::BaseDirectoriesError);
       Melib(melib::error::MeliError);
       Configuration(toml::de::Error);
       SerdeJson(serde_json::Error);
   }
}
