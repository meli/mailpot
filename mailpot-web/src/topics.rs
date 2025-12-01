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

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SearchTerm {
    query: Option<String>,
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct SearchResult {
    pk: i64,
    id: String,
    description: Option<String>,
    topics: Vec<String>,
}

impl std::fmt::Display for SearchResult {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(fmt, "{:?}", self)
    }
}

impl Object for SearchResult {
    fn kind(&self) -> minijinja::value::ObjectKind<'_> {
        minijinja::value::ObjectKind::Struct(self)
    }

    fn call_method(
        &self,
        _state: &minijinja::State,
        name: &str,
        _args: &[Value],
    ) -> std::result::Result<Value, Error> {
        match name {
            "topics_html" => crate::minijinja_utils::topics_common(&self.topics),
            _ => Err(Error::new(
                minijinja::ErrorKind::UnknownMethod,
                format!("object has no method named {name}"),
            )),
        }
    }
}

impl minijinja::value::StructObject for SearchResult {
    fn get_field(&self, name: &str) -> Option<Value> {
        match name {
            "pk" => Some(Value::from_serializable(&self.pk)),
            "id" => Some(Value::from_serializable(&self.id)),
            "description" => Some(
                self.description
                    .clone()
                    .map(Value::from_safe_string)
                    .unwrap_or_else(|| Value::from_serializable(&self.description)),
            ),
            "topics" => Some(Value::from_serializable(&self.topics)),
            _ => None,
        }
    }

    fn static_fields(&self) -> Option<&'static [&'static str]> {
        Some(&["pk", "id", "description", "topics"][..])
    }
}
pub async fn list_topics(
    _: TopicsPath,
    mut session: WritableSession,
    Query(SearchTerm { query: term }): Query<SearchTerm>,
    auth: AuthContext,
    State(state): State<Arc<AppState>>,
) -> Result<Html<String>, ResponseError> {
    let db = Connection::open_db(state.conf.clone())?.trusted();

    let results: Vec<Value> = {
        if let Some(term) = term.as_ref() {
            let mut stmt = db.connection.prepare(
                "SELECT DISTINCT list.pk, list.id, list.description, list.topics FROM list, \
                 json_each(list.topics) WHERE json_each.value IS ?;",
            )?;
            let iter = stmt.query_map([&term], |row| {
                let pk = row.get(0)?;
                let id = row.get(1)?;
                let description = row.get(2)?;
                let topics = mailpot::models::MailingList::topics_from_json_value(row.get(3)?)?;
                Ok(Value::from_object(SearchResult {
                    pk,
                    id,
                    description,
                    topics,
                }))
            })?;
            let mut ret = vec![];
            for el in iter {
                let el = el?;
                ret.push(el);
            }
            ret
        } else {
            db.lists()?
                .into_iter()
                .map(DbVal::into_inner)
                .map(|l| SearchResult {
                    pk: l.pk,
                    id: l.id,
                    description: l.description,
                    topics: l.topics,
                })
                .map(Value::from_object)
                .collect()
        }
    };

    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: "Search for topics".into(),
            url: TopicsPath.to_crumb(),
        },
    ];
    let context = minijinja::context! {
        canonical_url => TopicsPath.to_crumb(),
        term,
        results,
        page_title => "Topic Search Results",
        description => "",
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs,
    };
    Ok(Html(
        TEMPLATES.get_template("topics.html")?.render(context)?,
    ))
}
