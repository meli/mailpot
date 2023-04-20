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

use super::*;

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize)]
pub struct Crumb {
    pub label: Cow<'static, str>,
    #[serde(serialize_with = "to_safe_string")]
    pub url: Cow<'static, str>,
}

#[derive(Debug, Default, Hash, Copy, Clone, serde::Deserialize, serde::Serialize)]
pub enum Level {
    Success,
    #[default]
    Info,
    Warning,
    Error,
}

#[derive(Debug, Hash, Clone, serde::Deserialize, serde::Serialize)]
pub struct Message {
    pub message: Cow<'static, str>,
    #[serde(default)]
    pub level: Level,
}

impl Message {
    const MESSAGE_KEY: &str = "session-message";
}

pub trait SessionMessages {
    fn drain_messages(&mut self) -> Vec<Message>;
    fn add_message(&mut self, _: Message) -> Result<(), ResponseError>;
}

impl SessionMessages for WritableSession {
    fn drain_messages(&mut self) -> Vec<Message> {
        let ret = self.get(Message::MESSAGE_KEY).unwrap_or_default();
        self.remove(Message::MESSAGE_KEY);
        ret
    }

    fn add_message(&mut self, message: Message) -> Result<(), ResponseError> {
        let mut messages: Vec<Message> = self.get(Message::MESSAGE_KEY).unwrap_or_default();
        messages.push(message);
        self.insert(Message::MESSAGE_KEY, messages)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Hash)]
#[repr(transparent)]
pub struct IntPOST(pub i64);

impl serde::Serialize for IntPOST {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_i64(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for IntPOST {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct IntVisitor;

        impl<'de> serde::de::Visitor<'de> for IntVisitor {
            type Value = IntPOST;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("Int as a number or string")
            }

            fn visit_i64<E>(self, int: i64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(IntPOST(int))
            }

            fn visit_str<E>(self, int: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                int.parse().map(IntPOST).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_any(IntVisitor)
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq, PartialOrd, Hash)]
#[repr(transparent)]
pub struct BoolPOST(pub bool);

impl serde::Serialize for BoolPOST {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bool(self.0)
    }
}

impl<'de> serde::Deserialize<'de> for BoolPOST {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct BoolVisitor;

        impl<'de> serde::de::Visitor<'de> for BoolVisitor {
            type Value = BoolPOST;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("Bool as a boolean or \"true\" \"false\"")
            }

            fn visit_bool<E>(self, val: bool) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(BoolPOST(val))
            }

            fn visit_str<E>(self, val: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                val.parse().map(BoolPOST).map_err(serde::de::Error::custom)
            }
        }

        deserializer.deserialize_any(BoolVisitor)
    }
}

#[derive(Debug, Clone, serde::Deserialize)]
pub struct Next {
    #[serde(default, deserialize_with = "empty_string_as_none")]
    pub next: Option<String>,
}

impl Next {
    #[inline]
    pub fn or_else(self, cl: impl FnOnce() -> String) -> Redirect {
        self.next
            .map_or_else(|| Redirect::to(&cl()), |next| Redirect::to(&next))
    }
}

/// Serde deserialization decorator to map empty Strings to None,
fn empty_string_as_none<'de, D, T>(de: D) -> Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: std::str::FromStr,
    T::Err: std::fmt::Display,
{
    use serde::Deserialize;
    let opt = Option::<String>::deserialize(de)?;
    match opt.as_deref() {
        None | Some("") => Ok(None),
        Some(s) => std::str::FromStr::from_str(s)
            .map_err(serde::de::Error::custom)
            .map(Some),
    }
}

/// Serialize string to [`minijinja::value::Value`] with
/// [`minijinja::value::Value::from_safe_string`].
pub fn to_safe_string<S>(s: impl AsRef<str>, ser: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;
    let s = s.as_ref();
    Value::from_safe_string(s.to_string()).serialize(ser)
}

/// Serialize an optional string to [`minijinja::value::Value`] with
/// [`minijinja::value::Value::from_safe_string`].
pub fn to_safe_string_opt<S>(s: &Option<String>, ser: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    use serde::Serialize;
    s.as_ref()
        .map(|s| Value::from_safe_string(s.to_string()))
        .serialize(ser)
}
