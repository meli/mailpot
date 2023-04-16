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

//! Utils for templates with the [`minijinja`] crate.

use super::*;

lazy_static::lazy_static! {
    pub static ref TEMPLATES: Environment<'static> = {
        let mut env = Environment::new();
        macro_rules! add {
            (function $($id:ident),*$(,)?) => {
                $(env.add_function(stringify!($id), $id);)*
            };
            (filter $($id:ident),*$(,)?) => {
                $(env.add_filter(stringify!($id), $id);)*
            }
        }
        add!(function calendarize,
            login_path,
            logout_path,
            settings_path,
            help_path,
            list_path,
            list_settings_path,
            list_edit_path,
            list_post_path
        );
        add!(filter pluralize);
        env.set_source(Source::from_path("web/src/templates/"));

        env
    };
}

pub trait StripCarets {
    fn strip_carets(&self) -> &str;
}

impl StripCarets for &str {
    fn strip_carets(&self) -> &str {
        let mut self_ref = self.trim();
        if self_ref.starts_with('<') && self_ref.ends_with('>') {
            self_ref = &self_ref[1..self_ref.len().saturating_sub(1)];
        }
        self_ref
    }
}

#[derive(Debug, PartialEq, Eq, Clone, serde::Deserialize, serde::Serialize)]
pub struct MailingList {
    pub pk: i64,
    pub name: String,
    pub id: String,
    pub address: String,
    pub description: Option<String>,
    #[serde(serialize_with = "super::utils::to_safe_string_opt")]
    pub archive_url: Option<String>,
    pub inner: DbVal<mailpot::models::MailingList>,
}

impl From<DbVal<mailpot::models::MailingList>> for MailingList {
    fn from(val: DbVal<mailpot::models::MailingList>) -> Self {
        let DbVal(
            mailpot::models::MailingList {
                pk,
                name,
                id,
                address,
                description,
                archive_url,
            },
            _,
        ) = val.clone();

        Self {
            pk,
            name,
            id,
            address,
            description,
            archive_url,
            inner: val,
        }
    }
}

impl std::fmt::Display for MailingList {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
        self.id.fmt(fmt)
    }
}

impl Object for MailingList {
    fn kind(&self) -> minijinja::value::ObjectKind {
        minijinja::value::ObjectKind::Struct(self)
    }

    fn call_method(
        &self,
        _state: &minijinja::State,
        name: &str,
        _args: &[Value],
    ) -> std::result::Result<Value, Error> {
        match name {
            "subscription_mailto" => {
                Ok(Value::from_serializable(&self.inner.subscription_mailto()))
            }
            "unsubscription_mailto" => Ok(Value::from_serializable(
                &self.inner.unsubscription_mailto(),
            )),
            _ => Err(Error::new(
                minijinja::ErrorKind::UnknownMethod,
                format!("object has no method named {name}"),
            )),
        }
    }
}

impl minijinja::value::StructObject for MailingList {
    fn get_field(&self, name: &str) -> Option<Value> {
        match name {
            "pk" => Some(Value::from_serializable(&self.pk)),
            "name" => Some(Value::from_serializable(&self.name)),
            "id" => Some(Value::from_serializable(&self.id)),
            "address" => Some(Value::from_serializable(&self.address)),
            "description" => Some(Value::from_serializable(&self.description)),
            "archive_url" => Some(Value::from_serializable(&self.archive_url)),
            _ => None,
        }
    }

    fn static_fields(&self) -> Option<&'static [&'static str]> {
        Some(&["pk", "name", "id", "address", "description", "archive_url"][..])
    }
}

pub fn calendarize(
    _state: &minijinja::State,
    args: Value,
    hists: Value,
) -> std::result::Result<Value, Error> {
    use chrono::Month;

    macro_rules! month {
        ($int:expr) => {{
            let int = $int;
            match int {
                1 => Month::January.name(),
                2 => Month::February.name(),
                3 => Month::March.name(),
                4 => Month::April.name(),
                5 => Month::May.name(),
                6 => Month::June.name(),
                7 => Month::July.name(),
                8 => Month::August.name(),
                9 => Month::September.name(),
                10 => Month::October.name(),
                11 => Month::November.name(),
                12 => Month::December.name(),
                _ => unreachable!(),
            }
        }};
    }
    let month = args.as_str().unwrap();
    let hist = hists
        .get_item(&Value::from(month))?
        .as_seq()
        .unwrap()
        .iter()
        .map(|v| usize::try_from(v).unwrap())
        .collect::<Vec<usize>>();
    let sum: usize = hists
        .get_item(&Value::from(month))?
        .as_seq()
        .unwrap()
        .iter()
        .map(|v| usize::try_from(v).unwrap())
        .sum();
    let date = chrono::NaiveDate::parse_from_str(&format!("{}-01", month), "%F").unwrap();
    // Week = [Mon, Tue, Wed, Thu, Fri, Sat, Sun]
    Ok(minijinja::context! {
        month_name => month!(date.month()),
        month => month,
        month_int => date.month() as usize,
        year => date.year(),
        weeks => cal::calendarize_with_offset(date, 1),
        hist => hist,
        sum => sum,
    })
}

/// `pluralize` filter for [`minijinja`].
///
/// Returns a plural suffix if the value is not `1`, `"1"`, or an object of
/// length `1`. By default, the plural suffix is 's' and the singular suffix is
/// empty (''). You can specify a singular suffix as the first argument (or
/// `None`, for the default). You can specify a plural suffix as the second
/// argument (or `None`, for the default).
///
/// See the examples for the correct usage.
///
/// # Examples
///
/// ```rust
/// # use mailpot_web::pluralize;
/// # use minijinja::Environment;
///
/// let mut env = Environment::new();
/// env.add_filter("pluralize", pluralize);
/// for (num, s) in [
///     (0, "You have 0 messages."),
///     (1, "You have 1 message."),
///     (10, "You have 10 messages."),
/// ] {
///     assert_eq!(
///         &env.render_str(
///             "You have {{ num_messages }} message{{ num_messages|pluralize }}.",
///             minijinja::context! {
///                 num_messages => num,
///             }
///         )
///         .unwrap(),
///         s
///     );
/// }
///
/// for (num, s) in [
///     (0, "You have 0 walruses."),
///     (1, "You have 1 walrus."),
///     (10, "You have 10 walruses."),
/// ] {
///     assert_eq!(
///         &env.render_str(
///             r#"You have {{ num_walruses }} walrus{{ num_walruses|pluralize(None, "es") }}."#,
///             minijinja::context! {
///                 num_walruses => num,
///             }
///         )
///         .unwrap(),
///         s
///     );
/// }
///
/// for (num, s) in [
///     (0, "You have 0 cherries."),
///     (1, "You have 1 cherry."),
///     (10, "You have 10 cherries."),
/// ] {
///     assert_eq!(
///         &env.render_str(
///             r#"You have {{ num_cherries }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
///             minijinja::context! {
///                 num_cherries => num,
///             }
///         )
///         .unwrap(),
///         s
///     );
/// }
///
/// assert_eq!(
///     &env.render_str(
///         r#"You have {{ num_cherries|length }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
///         minijinja::context! {
///             num_cherries => vec![(); 5],
///         }
///     )
///     .unwrap(),
///     "You have 5 cherries."
/// );
///
/// assert_eq!(
///     &env.render_str(
///         r#"You have {{ num_cherries }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
///         minijinja::context! {
///             num_cherries => "5",
///         }
///     )
///     .unwrap(),
///     "You have 5 cherries."
/// );
/// assert_eq!(
///     &env.render_str(
///         r#"You have 1 cherr{{ num_cherries|pluralize("y", "ies") }}."#,
///         minijinja::context! {
///             num_cherries => true,
///         }
///     )
///     .unwrap()
///     .to_string(),
///     "You have 1 cherry.",
/// );
/// assert_eq!(
///     &env.render_str(
///         r#"You have {{ num_cherries }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
///         minijinja::context! {
///             num_cherries => 0.5f32,
///         }
///     )
///     .unwrap_err()
///     .to_string(),
///     "invalid operation: Pluralize argument is not an integer, or a sequence / object with a \
///      length but of type number (in <string>:1)",
/// );
/// ```
pub fn pluralize(
    v: Value,
    singular: Option<String>,
    plural: Option<String>,
) -> Result<Value, minijinja::Error> {
    macro_rules! int_try_from {
         ($ty:ty) => {
             <$ty>::try_from(v.clone()).ok().map(|v| v != 1)
         };
         ($fty:ty, $($ty:ty),*) => {
             int_try_from!($fty).or_else(|| int_try_from!($($ty),*))
         }
     }
    let is_plural: bool = v
        .as_str()
        .and_then(|s| s.parse::<i128>().ok())
        .map(|l| l != 1)
        .or_else(|| v.len().map(|l| l != 1))
        .or_else(|| int_try_from!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, usize))
        .ok_or_else(|| {
            minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!(
                    "Pluralize argument is not an integer, or a sequence / object with a length \
                     but of type {}",
                    v.kind()
                ),
            )
        })?;
    Ok(match (is_plural, singular, plural) {
        (false, None, _) => "".into(),
        (false, Some(suffix), _) => suffix.into(),
        (true, _, None) => "s".into(),
        (true, _, Some(suffix)) => suffix.into(),
    })
}
