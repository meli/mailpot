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

/// Navigation crumbs, e.g.: Home > Page > Subpage
///
/// # Example
///
/// ```rust
/// # use mailpot_web::utils::Crumb;
/// let crumbs = vec![Crumb {
///     label: "Home".into(),
///     url: "/".into(),
/// }];
/// println!("{} {}", crumbs[0].label, crumbs[0].url);
/// ```
#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize)]
pub struct Crumb {
    pub label: Cow<'static, str>,
    #[serde(serialize_with = "to_safe_string")]
    pub url: Cow<'static, str>,
}

/// Message urgency level or info.
#[derive(
    Debug, Default, Hash, Copy, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq,
)]
pub enum Level {
    Success,
    #[default]
    Info,
    Warning,
    Error,
}

/// UI message notifications.
#[derive(Debug, Hash, Clone, serde::Deserialize, serde::Serialize, PartialEq, Eq)]
pub struct Message {
    pub message: Cow<'static, str>,
    #[serde(default)]
    pub level: Level,
}

impl Message {
    const MESSAGE_KEY: &'static str = "session-message";
}

/// Drain messages from session.
///
/// # Example
///
/// ```no_run
/// # use mailpot_web::utils::{Message, Level, SessionMessages};
/// struct Session(Vec<Message>);
///
/// impl SessionMessages for Session {
///     type Error = std::convert::Infallible;
///     fn drain_messages(&mut self) -> Vec<Message> {
///         std::mem::take(&mut self.0)
///     }
///
///     fn add_message(&mut self, m: Message) -> Result<(), std::convert::Infallible> {
///         self.0.push(m);
///         Ok(())
///     }
/// }
/// let mut s = Session(vec![]);
/// s.add_message(Message {
///     message: "foo".into(),
///     level: Level::default(),
/// })
/// .unwrap();
/// s.add_message(Message {
///     message: "bar".into(),
///     level: Level::Error,
/// })
/// .unwrap();
/// assert_eq!(
///     s.drain_messages().as_slice(),
///     [
///         Message {
///             message: "foo".into(),
///             level: Level::default(),
///         },
///         Message {
///             message: "bar".into(),
///             level: Level::Error
///         }
///     ]
///     .as_slice()
/// );
/// assert!(s.0.is_empty());
/// ```
pub trait SessionMessages {
    type Error;

    fn drain_messages(&mut self) -> Vec<Message>;
    fn add_message(&mut self, _: Message) -> Result<(), Self::Error>;
}

impl SessionMessages for WritableSession {
    type Error = ResponseError;

    fn drain_messages(&mut self) -> Vec<Message> {
        let ret = self.get(Message::MESSAGE_KEY).unwrap_or_default();
        self.remove(Message::MESSAGE_KEY);
        ret
    }

    #[allow(clippy::significant_drop_tightening)]
    fn add_message(&mut self, message: Message) -> Result<(), ResponseError> {
        let mut messages: Vec<Message> = self.get(Message::MESSAGE_KEY).unwrap_or_default();
        messages.push(message);
        self.insert(Message::MESSAGE_KEY, messages)?;
        Ok(())
    }
}

/// Deserialize a string integer into `i64`, because POST parameters are
/// strings.
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

            fn visit_u64<E>(self, int: u64) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                Ok(IntPOST(int.try_into().unwrap()))
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

/// Deserialize a string integer into `bool`, because POST parameters are
/// strings.
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

pub struct ThreadEntry {
    pub hash: melib::EnvelopeHash,
    pub depth: usize,
    pub thread_node: melib::ThreadNodeHash,
    pub thread: melib::ThreadHash,
    pub from: String,
    pub message_id: String,
    pub timestamp: u64,
    pub datetime: String,
}

pub fn thread(
    envelopes: &Arc<std::sync::RwLock<HashMap<melib::EnvelopeHash, melib::Envelope>>>,
    threads: &melib::Threads,
    root_env_hash: melib::EnvelopeHash,
) -> Vec<ThreadEntry> {
    let env_lock = envelopes.read().unwrap();
    let thread = threads.envelope_to_thread[&root_env_hash];
    let mut ret = vec![];
    for (depth, t) in threads.thread_group_iter(thread) {
        let hash = threads.thread_nodes[&t].message.unwrap();
        ret.push(ThreadEntry {
            hash,
            depth,
            thread_node: t,
            thread,
            message_id: env_lock[&hash].message_id().to_string(),
            from: env_lock[&hash].field_from_to_string(),
            datetime: env_lock[&hash].date_as_str().to_string(),
            timestamp: env_lock[&hash].timestamp,
        });
    }
    ret
}

pub fn thread_roots(
    envelopes: &Arc<std::sync::RwLock<HashMap<melib::EnvelopeHash, melib::Envelope>>>,
    threads: &melib::Threads,
) -> Vec<(ThreadEntry, usize, u64)> {
    let items = threads.roots();
    let env_lock = envelopes.read().unwrap();
    let mut ret = vec![];
    'items_for_loop: for thread in items {
        let mut iter_ptr = threads.thread_ref(thread).root();
        let thread_node = &threads.thread_nodes()[&iter_ptr];
        let root_env_hash = if let Some(h) = thread_node.message().or_else(|| {
            if thread_node.children().is_empty() {
                return None;
            }
            iter_ptr = thread_node.children()[0];
            while threads.thread_nodes()[&iter_ptr].message().is_none() {
                if threads.thread_nodes()[&iter_ptr].children().is_empty() {
                    return None;
                }
                iter_ptr = threads.thread_nodes()[&iter_ptr].children()[0];
            }
            threads.thread_nodes()[&iter_ptr].message()
        }) {
            h
        } else {
            continue 'items_for_loop;
        };
        if !env_lock.contains_key(&root_env_hash) {
            panic!("key = {}", root_env_hash);
        }
        let envelope: &melib::Envelope = &env_lock[&root_env_hash];
        let tref = threads.thread_ref(thread);
        ret.push((
            ThreadEntry {
                hash: root_env_hash,
                depth: 0,
                thread_node: iter_ptr,
                thread,
                message_id: envelope.message_id().to_string(),
                from: envelope.field_from_to_string(),
                datetime: envelope.date_as_str().to_string(),
                timestamp: envelope.timestamp,
            },
            tref.len,
            tref.date,
        ));
    }
    // clippy: error: temporary with significant `Drop` can be early dropped
    drop(env_lock);
    ret.sort_by_key(|(_, _, key)| std::cmp::Reverse(*key));
    ret
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session() {
        struct Session(Vec<Message>);

        impl SessionMessages for Session {
            type Error = std::convert::Infallible;
            fn drain_messages(&mut self) -> Vec<Message> {
                std::mem::take(&mut self.0)
            }

            fn add_message(&mut self, m: Message) -> Result<(), std::convert::Infallible> {
                self.0.push(m);
                Ok(())
            }
        }
        let mut s = Session(vec![]);
        s.add_message(Message {
            message: "foo".into(),
            level: Level::default(),
        })
        .unwrap();
        s.add_message(Message {
            message: "bar".into(),
            level: Level::Error,
        })
        .unwrap();
        assert_eq!(
            s.drain_messages().as_slice(),
            [
                Message {
                    message: "foo".into(),
                    level: Level::default(),
                },
                Message {
                    message: "bar".into(),
                    level: Level::Error
                }
            ]
            .as_slice()
        );
        assert!(s.0.is_empty());
    }

    #[test]
    fn test_post_serde() {
        use mailpot::serde_json::{self, json};
        assert_eq!(
            IntPOST(5),
            serde_json::from_str::<IntPOST>("\"5\"").unwrap()
        );
        assert_eq!(IntPOST(5), serde_json::from_str::<IntPOST>("5").unwrap());
        assert_eq!(&json! { IntPOST(5) }.to_string(), "5");

        assert_eq!(
            BoolPOST(true),
            serde_json::from_str::<BoolPOST>("true").unwrap()
        );
        assert_eq!(
            BoolPOST(true),
            serde_json::from_str::<BoolPOST>("\"true\"").unwrap()
        );
        assert_eq!(&json! { BoolPOST(false) }.to_string(), "false");
    }

    #[test]
    fn test_next() {
        let next = Next {
            next: Some("foo".to_string()),
        };
        assert_eq!(
            format!("{:?}", Redirect::to("foo")),
            format!("{:?}", next.or_else(|| "bar".to_string()))
        );
        let next = Next { next: None };
        assert_eq!(
            format!("{:?}", Redirect::to("bar")),
            format!("{:?}", next.or_else(|| "bar".to_string()))
        );
    }
}
