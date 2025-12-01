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

use minijinja::{value::Value, Environment};

use crate::typed_paths::{
    help_path, list_candidates_path, list_edit_path, list_path, list_post_path, list_settings_path,
    list_subscribers_path, login_path, logout_path, post_eml_path, post_mbox_path, post_raw_path,
    settings_path,
};

mod compressed;
mod filters;
mod objects;

pub use filters::*;
pub use objects::*;

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
            strip_carets,
            ensure_carets,
            urlize,
            url_encode,
            heading,
            topics,
            login_path,
            logout_path,
            settings_path,
            help_path,
            list_path,
            list_settings_path,
            list_edit_path,
            list_subscribers_path,
            list_candidates_path,
            list_post_path,
            post_raw_path,
            post_eml_path,
            post_mbox_path,
            time_dur,
        );
        add!(filter pluralize);
        // Load compressed templates. They are constructed in build.rs. See
        // [ref:embed_templates]
        let mut source = minijinja::Source::new();
        for (name, bytes) in compressed::COMPRESSED {
            let mut de_bytes = vec![];
            zstd::stream::copy_decode(*bytes,&mut de_bytes).unwrap();
            source.add_template(*name, String::from_utf8(de_bytes).unwrap()).unwrap();
        }
        env.set_source(source);

        env.add_global("root_url_prefix", Value::from_safe_string(std::env::var("ROOT_URL_PREFIX").unwrap_or_default()));
        env.add_global("public_url", Value::from_safe_string(std::env::var("PUBLIC_URL").unwrap_or_else(|_| "localhost".to_string())));
        env.add_global("site_title", Value::from_safe_string(std::env::var("SITE_TITLE").unwrap_or_else(|_| "mailing list archive".to_string())));
        env.add_global("site_subtitle", std::env::var("SITE_SUBTITLE").ok().map(Value::from_safe_string).unwrap_or_default());

        env
    };
}
