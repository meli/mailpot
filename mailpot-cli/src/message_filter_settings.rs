// @generated
//
// This file is part of mailpot
//
// Copyright 2023 - Manos Pitsidianakis
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as
// published by the Free Software Foundation, either version 3 of the
// License, or (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program. If not, see <https://www.gnu.org/licenses/>.

use clap::builder::TypedValueParser;

# [allow (clippy :: enum_variant_names)] # [derive (Clone , Copy , Debug , PartialEq , Eq , Hash)] pub enum MessageFilterSettingName { AddSubjectTagPrefixSettings , MimeRejectSettings , ArchivedAtLinkSettings } impl :: std :: str :: FromStr for MessageFilterSettingName { type Err = String ; fn from_str (s : & str) -> Result < Self , Self :: Err > { # ! [allow (clippy :: suspicious_else_formatting)] if s . eq_ignore_ascii_case (stringify ! (AddSubjectTagPrefixSettings)) { return Ok (Self :: AddSubjectTagPrefixSettings) ; } if s . eq_ignore_ascii_case (stringify ! (MimeRejectSettings)) { return Ok (Self :: MimeRejectSettings) ; } if s . eq_ignore_ascii_case (stringify ! (ArchivedAtLinkSettings)) { return Ok (Self :: ArchivedAtLinkSettings) ; } Err (format ! ("Unrecognized value: {s}")) } } impl :: std :: fmt :: Display for MessageFilterSettingName { fn fmt (& self , fmt : & mut :: std :: fmt :: Formatter) -> :: std :: fmt :: Result { write ! (fmt , "{}" , match self { Self :: AddSubjectTagPrefixSettings => stringify ! (AddSubjectTagPrefixSettings) , Self :: MimeRejectSettings => stringify ! (MimeRejectSettings) , Self :: ArchivedAtLinkSettings => stringify ! (ArchivedAtLinkSettings) }) } }# [derive (Clone , Copy , Debug)] pub struct MessageFilterSettingNameValueParser ; impl MessageFilterSettingNameValueParser { pub fn new () -> Self { Self } } impl TypedValueParser for MessageFilterSettingNameValueParser { type Value = MessageFilterSettingName ; fn parse_ref (& self , cmd : & clap :: Command , arg : Option < & clap :: Arg > , value : & std :: ffi :: OsStr ,) -> std :: result :: Result < Self :: Value , clap :: Error > { TypedValueParser :: parse (self , cmd , arg , value . to_owned ()) } fn parse (& self , cmd : & clap :: Command , _arg : Option < & clap :: Arg > , value : std :: ffi :: OsString ,) -> std :: result :: Result < Self :: Value , clap :: Error > { use std :: str :: FromStr ; use clap :: error :: ErrorKind ; if value . is_empty () { return Err (cmd . clone () . error (ErrorKind :: DisplayHelpOnMissingArgumentOrSubcommand , "Message filter setting name value required" ,)) ; } Self :: Value :: from_str (value . to_str () . ok_or_else (|| { cmd . clone () . error (ErrorKind :: InvalidValue , "Message filter setting name value is not an UTF-8 string" ,) }) ?) . map_err (| err | cmd . clone () . error (ErrorKind :: InvalidValue , err)) } fn possible_values (& self) -> Option < Box < dyn Iterator < Item = clap :: builder :: PossibleValue >> > { Some (Box :: new (["AddSubjectTagPrefixSettings" , "MimeRejectSettings" , "ArchivedAtLinkSettings"] . iter () . map (clap :: builder :: PossibleValue :: new) ,)) } } impl Default for MessageFilterSettingNameValueParser { fn default () -> Self { Self :: new () } }
