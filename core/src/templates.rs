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

//! Template database model: [`Template`].

use super::*;

/// A named template.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
pub struct Template {
    /// Database primary key.
    pub pk: i64,
    /// Name.
    pub name: String,
    /// Associated list foreign key, optional.
    pub list: Option<i64>,
    /// Subject template.
    pub subject: Option<String>,
    /// Extra headers template.
    pub headers_json: Option<serde_json::Value>,
    /// Body template.
    pub body: String,
}

impl std::fmt::Display for Template {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

impl Template {
    /// Template name for generic list help e-mail.
    pub const GENERIC_HELP: &str = "generic-help";
    /// Template name for generic failure e-mail.
    pub const GENERIC_FAILURE: &str = "generic-failure";
    /// Template name for generic success e-mail.
    pub const GENERIC_SUCCESS: &str = "generic-success";
    /// Template name for subscription confirmation e-mail.
    pub const SUBSCRIPTION_CONFIRMATION: &str = "subscription-confirmation";
    /// Template name for unsubscription confirmation e-mail.
    pub const UNSUBSCRIPTION_CONFIRMATION: &str = "unsubscription-confirmation";
    /// Template name for subscription request notice e-mail (for list owners).
    pub const SUBSCRIPTION_REQUEST_NOTICE_OWNER: &str = "subscription-notice-owner";
    /// Template name for subscription request acceptance e-mail (for the
    /// candidates).
    pub const SUBSCRIPTION_REQUEST_CANDIDATE_ACCEPT: &str = "subscription-notice-candidate-accept";
    /// Template name for admin notices.
    pub const ADMIN_NOTICE: &str = "admin-notice";

    /// Render a message body from a saved named template.
    pub fn render(&self, context: minijinja::value::Value) -> Result<melib::Draft> {
        use melib::{Draft, HeaderName};

        let env = minijinja::Environment::new();
        let mut draft: Draft = Draft {
            body: env.render_named_str("body", &self.body, &context)?,
            ..Draft::default()
        };
        if let Some(ref subject) = self.subject {
            draft.headers.insert(
                HeaderName::new_unchecked("Subject"),
                env.render_named_str("subject", subject, &context)?,
            );
        }

        Ok(draft)
    }

    /// Template name for generic failure e-mail.
    pub fn default_generic_failure() -> Self {
        Self {
            pk: -1,
            name: Self::GENERIC_FAILURE.to_string(),
            list: None,
            subject: Some(
                "{{ subject if subject else \"Your e-mail was not processed successfully.\" }}"
                    .to_string(),
            ),
            headers_json: None,
            body: "{{ details|safe if details else \"The list owners and administrators have been \
                   notified.\" }}"
                .to_string(),
        }
    }

    /// Create a plain template for generic success e-mails.
    pub fn default_generic_success() -> Self {
        Self {
            pk: -1,
            name: Self::GENERIC_SUCCESS.to_string(),
            list: None,
            subject: Some(
                "{{ subject if subject else \"Your e-mail was processed successfully.\" }}"
                    .to_string(),
            ),
            headers_json: None,
            body: "{{ details|safe if details else \"\" }}".to_string(),
        }
    }

    /// Create a plain template for subscription confirmation.
    pub fn default_subscription_confirmation() -> Self {
        Self {
            pk: -1,
            name: Self::SUBSCRIPTION_CONFIRMATION.to_string(),
            list: None,
            subject: Some(
                "{% if list and (list.id or list.name) %}{% if list.id %}[{{ list.id }}] {% endif \
                 %}You have successfully subscribed to {{ list.name if list.name else list.id \
                 }}{% else %}You have successfully subscribed to this list{% endif %}."
                    .to_string(),
            ),
            headers_json: None,
            body: "{{ details|safe if details else \"\" }}".to_string(),
        }
    }

    /// Create a plain template for unsubscription confirmations.
    pub fn default_unsubscription_confirmation() -> Self {
        Self {
            pk: -1,
            name: Self::UNSUBSCRIPTION_CONFIRMATION.to_string(),
            list: None,
            subject: Some(
                "{% if list and (list.id or list.name) %}{% if list.id %}[{{ list.id }}] {% endif \
                 %}You have successfully unsubscribed from {{ list.name if list.name else list.id \
                 }}{% else %}You have successfully unsubscribed from this list{% endif %}."
                    .to_string(),
            ),
            headers_json: None,
            body: "{{ details|safe if details else \"\" }}".to_string(),
        }
    }

    /// Create a plain template for admin notices.
    pub fn default_admin_notice() -> Self {
        Self {
            pk: -1,
            name: Self::ADMIN_NOTICE.to_string(),
            list: None,
            subject: Some(
                "{% if list %}An error occured with list {{ list.id }}{% else %}An error \
                 occured{% endif %}"
                    .to_string(),
            ),
            headers_json: None,
            body: "{{ details|safe if details else \"\" }}".to_string(),
        }
    }

    /// Create a plain template for subscription requests for list owners.
    pub fn default_subscription_request_owner() -> Self {
        Self {
            pk: -1,
            name: Self::SUBSCRIPTION_REQUEST_NOTICE_OWNER.to_string(),
            list: None,
            subject: Some("Subscription request for {{ list.id }} by {{ candidate }}".to_string()),
            headers_json: None,
            body: "Candidate primary key: {{ candidate.pk }}\n\n{{ details|safe if details else \
                   \"\" }}"
                .to_string(),
        }
    }

    /// Create a plain template for subscription requests for candidates.
    pub fn default_subscription_request_candidate_accept() -> Self {
        Self {
            pk: -1,
            name: Self::SUBSCRIPTION_REQUEST_CANDIDATE_ACCEPT.to_string(),
            list: None,
            subject: Some("Your subscription to {{ list.id }} is now active.".to_string()),
            headers_json: None,
            body: "{{ details|safe if details else \"\" }}".to_string(),
        }
    }

    /// Create a plain template for generic list help replies.
    pub fn default_generic_help() -> Self {
        Self {
            pk: -1,
            name: Self::GENERIC_HELP.to_string(),
            list: None,
            subject: Some("{{ subject if subject else \"Help for mailing list\" }}".to_string()),
            headers_json: None,
            body: "{{ details }}".to_string(),
        }
    }
}
