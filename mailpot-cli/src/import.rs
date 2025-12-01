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

//! Import members in a local list from a remote mailman3 REST API instance with
//! `mpot list import-members ...`.

use std::{borrow::Cow, time::Duration};

use base64::{engine::general_purpose, Engine as _};
use mailpot::models::{ListOwner, ListSubscription};
use ureq::Agent;

pub struct Mailman3Connection {
    agent: Agent,
    url: Cow<'static, str>,
    auth: String,
}

impl Mailman3Connection {
    pub fn new(
        url: &str,
        username: &str,
        password: &str,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let agent: Agent = ureq::AgentBuilder::new()
            .timeout_read(Duration::from_secs(5))
            .timeout_write(Duration::from_secs(5))
            .build();
        let mut buf = String::new();
        general_purpose::STANDARD
            .encode_string(format!("{username}:{password}").as_bytes(), &mut buf);

        let auth: String = format!("Basic {buf}");

        Ok(Self {
            agent,
            url: url.trim_end_matches('/').to_string().into(),
            auth,
        })
    }

    pub fn users(&self, list_address: &str) -> Result<Vec<Entry>, Box<dyn std::error::Error>> {
        let response: String = self
            .agent
            .get(&format!(
                "{}/lists/{list_address}/roster/member?fields=email&fields=display_name",
                self.url
            ))
            .set("Authorization", &self.auth)
            .call()?
            .into_string()?;
        Ok(serde_json::from_str::<Roster>(&response)?.entries)
    }

    pub fn owners(&self, list_address: &str) -> Result<Vec<Entry>, Box<dyn std::error::Error>> {
        let response: String = self
            .agent
            .get(&format!(
                "{}/lists/{list_address}/roster/owner?fields=email&fields=display_name",
                self.url
            ))
            .set("Authorization", &self.auth)
            .call()?
            .into_string()?;
        Ok(serde_json::from_str::<Roster>(&response)?.entries)
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct Roster {
    pub entries: Vec<Entry>,
}

#[derive(serde::Deserialize, Debug)]
pub struct Entry {
    display_name: String,
    email: String,
}

impl Entry {
    pub fn display_name(&self) -> Option<&str> {
        if !self.display_name.trim().is_empty() && &self.display_name != "None" {
            Some(&self.display_name)
        } else {
            None
        }
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn into_subscription(self, list: i64) -> ListSubscription {
        let Self {
            display_name,
            email,
        } = self;

        ListSubscription {
            pk: -1,
            list,
            address: email,
            name: if !display_name.trim().is_empty() && &display_name != "None" {
                Some(display_name)
            } else {
                None
            },
            account: None,
            enabled: true,
            verified: true,
            digest: false,
            hide_address: false,
            receive_duplicates: false,
            receive_own_posts: false,
            receive_confirmation: false,
        }
    }

    pub fn into_owner(self, list: i64) -> ListOwner {
        let Self {
            display_name,
            email,
        } = self;

        ListOwner {
            pk: -1,
            list,
            address: email,
            name: if !display_name.trim().is_empty() && &display_name != "None" {
                Some(display_name)
            } else {
                None
            },
        }
    }
}
