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

/// Show help page.
pub async fn help(
    _: HelpPath,
    mut session: WritableSession,
    auth: AuthContext,
) -> Result<Html<String>, ResponseError> {
    let crumbs = vec![
        Crumb {
            label: "Home".into(),
            url: "/".into(),
        },
        Crumb {
            label: "Help".into(),
            url: HelpPath.to_crumb(),
        },
    ];
    let context = minijinja::context! {
        page_title => "Help & Documentation",
        current_user => auth.current_user,
        messages => session.drain_messages(),
        crumbs => crumbs,
    };
    Ok(Html(TEMPLATES.get_template("help.html")?.render(context)?))
}
