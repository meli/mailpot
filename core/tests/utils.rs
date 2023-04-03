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

use std::sync::Once;

static INIT_STDERR_LOGGING: Once = Once::new();

pub fn init_stderr_logging() {
    INIT_STDERR_LOGGING.call_once(|| {
        stderrlog::new()
            .quiet(false)
            .verbosity(15)
            .show_module_names(true)
            .timestamp(stderrlog::Timestamp::Millisecond)
            .init()
            .unwrap();
    });
}
