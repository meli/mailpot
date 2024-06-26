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

use std::fmt::Write;

pub use mailpot::StripCarets;

use super::*;

/// Return a vector of weeks, with each week being a vector of 7 days and
/// corresponding sum of posts per day.
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
        sum,
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
/// ```rust,no_run
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

/// `strip_carets` filter for [`minijinja`].
///
/// Removes `<`, `>` from message ids.
pub fn strip_carets(_state: &minijinja::State, arg: Value) -> std::result::Result<Value, Error> {
    Ok(Value::from(
        arg.as_str()
            .ok_or_else(|| {
                minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    format!("argument to strip_carets() is of type {}", arg.kind()),
                )
            })?
            .strip_carets(),
    ))
}

/// `ensure_carets` filter for [`minijinja`].
///
/// Makes sure message id value is surrounded by carets `<', `>`.
pub fn ensure_carets(_state: &minijinja::State, arg: Value) -> std::result::Result<Value, Error> {
    Ok({
        let s = arg.as_str().ok_or_else(|| {
            minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!("argument to ensure_carets() is of type {}", arg.kind()),
            )
        })?;
        if !s.trim().starts_with('<') && !s.ends_with('>') {
            Value::from(format!("<{s}>"))
        } else {
            Value::from(s)
        }
    })
}

/// `urlize` filter for [`minijinja`].
///
/// Returns a safe string for use in `<a href=..` attributes.
///
/// # Examples
///
/// ```rust,no_run
/// # use mailpot_web::urlize;
/// # use minijinja::Environment;
/// # use minijinja::value::Value;
///
/// let mut env = Environment::new();
/// env.add_function("urlize", urlize);
/// env.add_global(
///     "root_url_prefix",
///     Value::from_safe_string("/lists/prefix/".to_string()),
/// );
/// assert_eq!(
///     &env.render_str(
///         "<a href=\"{{ urlize(\"path/index.html\") }}\">link</a>",
///         minijinja::context! {}
///     )
///     .unwrap(),
///     "<a href=\"/lists/prefix/path/index.html\">link</a>",
/// );
/// ```
pub fn urlize(state: &minijinja::State, arg: Value) -> std::result::Result<Value, Error> {
    let Some(prefix) = state.lookup("root_url_prefix") else {
        return Ok(arg);
    };
    Ok(Value::from_safe_string(format!("{prefix}{arg}")))
}

pub fn url_encode(_state: &minijinja::State, arg: Value) -> std::result::Result<Value, Error> {
    Ok(Value::from_safe_string(
        utf8_percent_encode(
            arg.as_str().ok_or_else(|| {
                minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    format!(
                        "url_decode() argument is not a string but of type {}",
                        arg.kind()
                    ),
                )
            })?,
            crate::typed_paths::PATH_SEGMENT,
        )
        .to_string(),
    ))
}

/// Make an html heading: `h1, h2, h3` etc.
///
/// # Example
/// ```rust,no_run
/// use mailpot_web::minijinja_utils::heading;
/// use minijinja::value::Value;
///
/// assert_eq!(
///   "<h1 id=\"bl-bfa-b-ah-b-asdb-hadas-d\">bl bfa B AH bAsdb hadas d<a class=\"self-link\" href=\"#bl-bfa-b-ah-b-asdb-hadas-d\"></a></h1>",
///   &heading(1.into(), "bl bfa B AH bAsdb hadas d".into(), None).unwrap().to_string()
/// );
/// assert_eq!(
///     "<h2 id=\"short\">bl bfa B AH bAsdb hadas d<a class=\"self-link\" href=\"#short\"></a></h2>",
///     &heading(2.into(), "bl bfa B AH bAsdb hadas d".into(), Some("short".into())).unwrap().to_string()
/// );
/// assert_eq!(
///     r#"invalid operation: first heading() argument must be an unsigned integer less than 7 and positive"#,
///     &heading(0.into(), "bl bfa B AH bAsdb hadas d".into(), Some("short".into())).unwrap_err().to_string()
/// );
/// assert_eq!(
///     r#"invalid operation: first heading() argument must be an unsigned integer less than 7 and positive"#,
///     &heading(8.into(), "bl bfa B AH bAsdb hadas d".into(), Some("short".into())).unwrap_err().to_string()
/// );
/// assert_eq!(
///     r#"invalid operation: first heading() argument is not an integer < 7 but of type sequence"#,
///     &heading(Value::from(vec![Value::from(1)]), "bl bfa B AH bAsdb hadas d".into(), Some("short".into())).unwrap_err().to_string()
/// );
/// ```
pub fn heading(level: Value, text: Value, id: Option<Value>) -> std::result::Result<Value, Error> {
    use convert_case::{Case, Casing};
    macro_rules! test {
        () => {
            |n| *n > 0 && *n < 7
        };
    }

    macro_rules! int_try_from {
         ($ty:ty) => {
             <$ty>::try_from(level.clone()).ok().filter(test!{}).map(|n| n as u8)
         };
         ($fty:ty, $($ty:ty),*) => {
             int_try_from!($fty).or_else(|| int_try_from!($($ty),*))
         }
     }
    let level: u8 = level
        .as_str()
        .and_then(|s| s.parse::<i128>().ok())
        .filter(test! {})
        .map(|n| n as u8)
        .or_else(|| int_try_from!(u8, u16, u32, u64, u128, i8, i16, i32, i64, i128, usize))
        .ok_or_else(|| {
            if matches!(level.kind(), minijinja::value::ValueKind::Number) {
                minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    "first heading() argument must be an unsigned integer less than 7 and positive",
                )
            } else {
                minijinja::Error::new(
                    minijinja::ErrorKind::InvalidOperation,
                    format!(
                        "first heading() argument is not an integer < 7 but of type {}",
                        level.kind()
                    ),
                )
            }
        })?;
    let text = text.as_str().ok_or_else(|| {
        minijinja::Error::new(
            minijinja::ErrorKind::InvalidOperation,
            format!(
                "second heading() argument is not a string but of type {}",
                text.kind()
            ),
        )
    })?;
    if let Some(v) = id {
        let kebab = v.as_str().ok_or_else(|| {
            minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                format!(
                    "third heading() argument is not a string but of type {}",
                    v.kind()
                ),
            )
        })?;
        Ok(Value::from_safe_string(format!(
            "<h{level} id=\"{kebab}\">{text}<a class=\"self-link\" \
             href=\"#{kebab}\"></a></h{level}>"
        )))
    } else {
        let kebab_v = text.to_case(Case::Kebab);
        let kebab =
            percent_encoding::utf8_percent_encode(&kebab_v, crate::typed_paths::PATH_SEGMENT);
        Ok(Value::from_safe_string(format!(
            "<h{level} id=\"{kebab}\">{text}<a class=\"self-link\" \
             href=\"#{kebab}\"></a></h{level}>"
        )))
    }
}

/// Make an array of topic strings into html badges.
///
/// # Example
/// ```rust
/// use mailpot_web::minijinja_utils::topics;
/// use minijinja::value::Value;
///
/// let v: Value = topics(Value::from_serializable(&vec![
///     "a".to_string(),
///     "aab".to_string(),
///     "aaab".to_string(),
/// ]))
/// .unwrap();
/// assert_eq!(
///     "<ul class=\"tags\"><li class=\"tag\" style=\"--red:110;--green:120;--blue:180;\"><span \
///      class=\"tag-name\"><a href=\"/topics/?query=a\">a</a></span></li><li class=\"tag\" \
///      style=\"--red:110;--green:120;--blue:180;\"><span class=\"tag-name\"><a \
///      href=\"/topics/?query=aab\">aab</a></span></li><li class=\"tag\" \
///      style=\"--red:110;--green:120;--blue:180;\"><span class=\"tag-name\"><a \
///      href=\"/topics/?query=aaab\">aaab</a></span></li></ul>",
///     &v.to_string()
/// );
/// ```
pub fn topics(topics: Value) -> std::result::Result<Value, Error> {
    topics.try_iter()?;
    let topics: Vec<String> = topics
        .try_iter()?
        .map(|v| v.to_string())
        .collect::<Vec<String>>();
    topics_common(&topics)
}

pub fn topics_common(topics: &[String]) -> std::result::Result<Value, Error> {
    let mut ul = String::new();
    write!(&mut ul, r#"<ul class="tags">"#)?;
    for topic in topics {
        write!(
            &mut ul,
            r#"<li class="tag" style="--red:110;--green:120;--blue:180;"><span class="tag-name"><a href=""#
        )?;
        write!(&mut ul, "{}", TopicsPath)?;
        write!(&mut ul, r#"?query="#)?;
        write!(
            &mut ul,
            "{}",
            utf8_percent_encode(topic, crate::typed_paths::PATH_SEGMENT)
        )?;
        write!(&mut ul, r#"">"#)?;
        write!(&mut ul, "{}", topic)?;
        write!(&mut ul, r#"</a></span></li>"#)?;
    }
    write!(&mut ul, r#"</ul>"#)?;
    Ok(Value::from_safe_string(ul))
}

#[cfg(test)]
mod tests {
    use mailpot::models::ListOwner;

    use super::*;

    #[test]
    fn test_pluralize() {
        let mut env = Environment::new();
        env.add_filter("pluralize", pluralize);
        for (num, s) in [
            (0, "You have 0 messages."),
            (1, "You have 1 message."),
            (10, "You have 10 messages."),
        ] {
            assert_eq!(
                &env.render_str(
                    "You have {{ num_messages }} message{{ num_messages|pluralize }}.",
                    minijinja::context! {
                        num_messages => num,
                    }
                )
                .unwrap(),
                s
            );
        }

        for (num, s) in [
            (0, "You have 0 walruses."),
            (1, "You have 1 walrus."),
            (10, "You have 10 walruses."),
        ] {
            assert_eq!(
        &env.render_str(
            r#"You have {{ num_walruses }} walrus{{ num_walruses|pluralize(None, "es") }}."#,
            minijinja::context! {
                num_walruses => num,
            }
        )
        .unwrap(),
        s
    );
        }

        for (num, s) in [
            (0, "You have 0 cherries."),
            (1, "You have 1 cherry."),
            (10, "You have 10 cherries."),
        ] {
            assert_eq!(
                &env.render_str(
                    r#"You have {{ num_cherries }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
                    minijinja::context! {
                        num_cherries => num,
                    }
                )
                .unwrap(),
                s
            );
        }

        assert_eq!(
    &env.render_str(
        r#"You have {{ num_cherries|length }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
        minijinja::context! {
            num_cherries => vec![(); 5],
        }
    )
    .unwrap(),
    "You have 5 cherries."
);

        assert_eq!(
            &env.render_str(
                r#"You have {{ num_cherries }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
                minijinja::context! {
                    num_cherries => "5",
                }
            )
            .unwrap(),
            "You have 5 cherries."
        );
        assert_eq!(
            &env.render_str(
                r#"You have 1 cherr{{ num_cherries|pluralize("y", "ies") }}."#,
                minijinja::context! {
                    num_cherries => true,
                }
            )
            .unwrap(),
            "You have 1 cherry.",
        );
        assert_eq!(
            &env.render_str(
                r#"You have {{ num_cherries }} cherr{{ num_cherries|pluralize("y", "ies") }}."#,
                minijinja::context! {
                    num_cherries => 0.5f32,
                }
            )
            .unwrap_err()
            .to_string(),
            "invalid operation: Pluralize argument is not an integer, or a sequence / object with \
             a length but of type number (in <string>:1)",
        );
    }

    #[test]
    fn test_urlize() {
        let mut env = Environment::new();
        env.add_function("urlize", urlize);
        env.add_global(
            "root_url_prefix",
            Value::from_safe_string("/lists/prefix/".to_string()),
        );
        assert_eq!(
            &env.render_str(
                "<a href=\"{{ urlize(\"path/index.html\") }}\">link</a>",
                minijinja::context! {}
            )
            .unwrap(),
            "<a href=\"/lists/prefix/path/index.html\">link</a>",
        );
    }

    #[test]
    fn test_heading() {
        assert_eq!(
            "<h1 id=\"bl-bfa-b-ah-b-asdb-hadas-d\">bl bfa B AH bAsdb hadas d<a \
             class=\"self-link\" href=\"#bl-bfa-b-ah-b-asdb-hadas-d\"></a></h1>",
            &heading(1.into(), "bl bfa B AH bAsdb hadas d".into(), None)
                .unwrap()
                .to_string()
        );
        assert_eq!(
            "<h2 id=\"short\">bl bfa B AH bAsdb hadas d<a class=\"self-link\" \
             href=\"#short\"></a></h2>",
            &heading(
                2.into(),
                "bl bfa B AH bAsdb hadas d".into(),
                Some("short".into())
            )
            .unwrap()
            .to_string()
        );
        assert_eq!(
            r#"invalid operation: first heading() argument must be an unsigned integer less than 7 and positive"#,
            &heading(
                0.into(),
                "bl bfa B AH bAsdb hadas d".into(),
                Some("short".into())
            )
            .unwrap_err()
            .to_string()
        );
        assert_eq!(
            r#"invalid operation: first heading() argument must be an unsigned integer less than 7 and positive"#,
            &heading(
                8.into(),
                "bl bfa B AH bAsdb hadas d".into(),
                Some("short".into())
            )
            .unwrap_err()
            .to_string()
        );
        assert_eq!(
            r#"invalid operation: first heading() argument is not an integer < 7 but of type sequence"#,
            &heading(
                Value::from(vec![Value::from(1)]),
                "bl bfa B AH bAsdb hadas d".into(),
                Some("short".into())
            )
            .unwrap_err()
            .to_string()
        );
    }

    #[test]
    fn test_strip_carets() {
        let mut env = Environment::new();
        env.add_filter("strip_carets", strip_carets);
        assert_eq!(
            &env.render_str(
                "{{ msg_id | strip_carets }}",
                minijinja::context! {
                    msg_id => "<hello1@example.com>",
                }
            )
            .unwrap(),
            "hello1@example.com",
        );
    }

    #[test]
    fn test_calendarize() {
        use std::collections::HashMap;

        let mut env = Environment::new();
        env.add_function("calendarize", calendarize);

        let month = "2001-09";
        let mut hist = [0usize; 31];
        hist[15] = 5;
        hist[1] = 1;
        hist[0] = 512;
        hist[30] = 30;
        assert_eq!(
    &env.render_str(
        "{% set c=calendarize(month, hists) %}Month: {{ c.month }} Month Name: {{ \
         c.month_name }} Month Int: {{ c.month_int }} Year: {{ c.year }} Sum: {{ c.sum }} {% \
         for week in c.weeks %}{% for day in week %}{% set num = c.hist[day-1] %}({{ day }}, \
         {{ num }}){% endfor %}{% endfor %}",
        minijinja::context! {
        month,
        hists => vec![(month.to_string(), hist)].into_iter().collect::<HashMap<String, [usize;
        31]>>(),
        }
    )
    .unwrap(),
    "Month: 2001-09 Month Name: September Month Int: 9 Year: 2001 Sum: 548 (0, 30)(0, 30)(0, \
     30)(0, 30)(0, 30)(1, 512)(2, 1)(3, 0)(4, 0)(5, 0)(6, 0)(7, 0)(8, 0)(9, 0)(10, 0)(11, \
     0)(12, 0)(13, 0)(14, 0)(15, 0)(16, 5)(17, 0)(18, 0)(19, 0)(20, 0)(21, 0)(22, 0)(23, \
     0)(24, 0)(25, 0)(26, 0)(27, 0)(28, 0)(29, 0)(30, 0)"
);
    }

    #[test]
    fn test_list_html_safe() {
        let mut list = MailingList {
            pk: 0,
            name: String::new(),
            id: String::new(),
            address: String::new(),
            description: None,
            topics: vec![],
            archive_url: None,
            inner: DbVal(
                mailpot::models::MailingList {
                    pk: 0,
                    name: String::new(),
                    id: String::new(),
                    address: String::new(),
                    description: None,
                    topics: vec![],
                    archive_url: None,
                },
                0,
            ),
            is_description_html_safe: false,
        };

        let mut list_owners = vec![ListOwner {
            pk: 0,
            list: 0,
            address: "admin@example.com".to_string(),
            name: None,
        }];
        let administrators = vec!["admin@example.com".to_string()];
        list.set_safety(&list_owners, &administrators);
        assert!(list.is_description_html_safe);
        list.set_safety::<ListOwner>(&[], &[]);
        assert!(list.is_description_html_safe);
        list.is_description_html_safe = false;
        list_owners[0].address = "user@example.com".to_string();
        list.set_safety(&list_owners, &administrators);
        assert!(!list.is_description_html_safe);
    }
}
