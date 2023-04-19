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

use std::{
    collections::{hash_map::RandomState, HashSet, VecDeque},
    hash::{BuildHasher, Hasher},
    io::Write,
};

use clap::ArgAction;
use clap_mangen::{roff, Man};
use roff::{bold, italic, roman, Inline, Roff};

include!("src/args.rs");

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=./src/args.rs");
    println!("cargo:rerun-if-changed=./build.rs");
    std::env::set_current_dir("..").expect("could not chdir('..')");

    let out_dir = PathBuf::from("./docs/");

    let cmd = Opt::command();

    let man = Man::new(cmd.clone()).title("mpot");
    let mut buffer: Vec<u8> = Default::default();
    man.render_title(&mut buffer)?;
    man.render_name_section(&mut buffer)?;
    man.render_synopsis_section(&mut buffer)?;
    man.render_description_section(&mut buffer)?;

    let mut roff = Roff::default();
    options(&mut roff, &cmd);
    roff.to_writer(&mut buffer)?;

    render_quick_start_section(&mut buffer)?;
    render_subcommands_section(&mut buffer)?;

    let mut visited = HashSet::new();

    let mut stack = VecDeque::new();
    let mut order = VecDeque::new();
    stack.push_back(vec![&cmd]);
    let s = RandomState::new();

    'stack: while let Some(cmds) = stack.pop_front() {
        for sub in cmds.last().unwrap().get_subcommands() {
            let mut hasher = s.build_hasher();
            for c in cmds.iter() {
                hasher.write(c.get_name().as_bytes());
            }
            hasher.write(sub.get_name().as_bytes());
            if visited.insert(hasher.finish()) {
                let mut sub_cmds = cmds.clone();
                sub_cmds.push(sub);
                order.push_back(sub_cmds.clone());
                stack.push_front(cmds);
                stack.push_front(sub_cmds);
                continue 'stack;
            }
        }
    }

    while let Some(mut subs) = order.pop_front() {
        let sub = subs.pop().unwrap();
        render_subcommand(&subs, sub, &mut buffer)?;
    }

    man.render_authors_section(&mut buffer)?;

    std::fs::write(out_dir.join("mpot.1"), buffer)?;

    Ok(())
}

fn render_quick_start_section(w: &mut dyn Write) -> Result<(), std::io::Error> {
    let mut roff = Roff::default();
    let heading = "QUICK START";
    roff.control("SH", [heading]);
    let tutorial = r#"mailpot saves its data in a sqlite3 file. To define the location of the sqlite3 file we need a configuration file, which can be generated with:

    mpot sample-config > conf.toml

Mailing lists can now be created:

    mpot -c conf.toml create-list --name "my first list" --id mylist --address mylist@example.com

You can list all the mailing lists with:

    mpot -c conf.toml list-lists

You should add yourself as the list owner:

    mpot -c conf.toml list mylist add-list-owner --address myself@example.com --name "Nemo"

And also enable posting and subscriptions by setting list policies:

    mpot -c conf.toml list mylist add-policy --subscriber-only

    mpot -c conf.toml list mylist add-subscribe-policy --request --send-confirmation

To post on a mailing list or submit a list request, pipe a raw e-mail into STDIN:

    mpot -c conf.toml post

You can configure your mail server to redirect e-mails addressed to your mailing lists to this command.

For postfix, you can automatically generate this configuration with:

    mpot -c conf.toml print-postfix-config --user myself --binary-path /path/to/mpot

This will print the following:

      - content of `transport_maps` and `local_recipient_maps`

        The output must be saved in a plain text file.
        Map output should be added to transport_maps and local_recipient_maps parameters in postfix's main.cf.
        To make postfix be able to read them, the postmap application must be executed with the
        path to the map file as its sole argument.

        postmap /path/to/mylist_maps

        postmap is usually distributed along with the other postfix binaries.

      - `master.cf` service entry
         The output must be entered in the master.cf file.
         See <https://www.postfix.org/master.5.html>.

"#;
    for line in tutorial.lines() {
        roff.text([roman(line.trim())]);
    }
    roff.to_writer(w)
}
fn render_subcommands_section(w: &mut dyn Write) -> Result<(), std::io::Error> {
    let mut roff = Roff::default();
    let heading = "SUBCOMMANDS";
    roff.control("SH", [heading]);
    roff.to_writer(w)
}

fn render_subcommand(
    parents: &[&clap::Command],
    sub: &clap::Command,
    w: &mut dyn Write,
) -> Result<(), std::io::Error> {
    let mut roff = Roff::default();
    _render_subcommand_full(parents, sub, &mut roff);
    options(&mut roff, sub);
    roff.to_writer(w)
}

fn _render_subcommand_full(parents: &[&clap::Command], sub: &clap::Command, roff: &mut Roff) {
    roff.control("\\fB", []);
    roff.control(
        "SS",
        parents
            .iter()
            .map(|cmd| cmd.get_name())
            .chain(std::iter::once(sub.get_name()))
            .collect::<Vec<_>>(),
    );
    roff.control("\\fR", []);
    roff.text([Inline::LineBreak]);

    synopsis(roff, parents, sub);
    roff.text([Inline::LineBreak]);

    if let Some(about) = sub.get_about().or_else(|| sub.get_long_about()) {
        let about = about.to_string();
        let mut iter = about.lines();
        let last = iter.nth_back(0);
        for line in iter {
            roff.text([roman(line.trim())]);
        }
        if let Some(line) = last {
            roff.text([roman(format!("{}.", line.trim()))]);
        }
    }
}

fn synopsis(roff: &mut Roff, parents: &[&clap::Command], sub: &clap::Command) {
    let mut line = parents
        .iter()
        .flat_map(|cmd| vec![roman(cmd.get_name()), roman(" ")].into_iter())
        .chain(std::iter::once(roman(sub.get_name())))
        .chain(std::iter::once(roman(" ")))
        .collect::<Vec<_>>();
    let arguments = sub
        .get_arguments()
        .filter(|i| !i.is_hide_set())
        .collect::<Vec<_>>();
    if arguments.is_empty() && sub.get_positionals().count() == 0 {
        return;
    }

    roff.text([Inline::LineBreak]);

    for opt in arguments {
        match (opt.get_short(), opt.get_long()) {
            (Some(short), Some(long)) => {
                let (lhs, rhs) = option_markers(opt);
                line.push(roman(lhs));
                line.push(roman(format!("-{short}")));
                if let Some(value) = opt.get_value_names() {
                    line.push(roman(" "));
                    line.push(italic(value.join(" ")));
                }

                line.push(roman("|"));
                line.push(roman(format!("--{long}",)));
                line.push(roman(rhs));
            }
            (Some(short), None) => {
                let (lhs, rhs) = option_markers_single(opt);
                line.push(roman(lhs));
                line.push(roman(format!("-{short}")));
                if let Some(value) = opt.get_value_names() {
                    line.push(roman(" "));
                    line.push(italic(value.join(" ")));
                }
                line.push(roman(rhs));
            }
            (None, Some(long)) => {
                let (lhs, rhs) = option_markers_single(opt);
                line.push(roman(lhs));
                line.push(roman(format!("--{long}")));
                if let Some(value) = opt.get_value_names() {
                    line.push(roman(" "));
                    line.push(italic(value.join(" ")));
                }
                line.push(roman(rhs));
            }
            (None, None) => continue,
        };

        if matches!(opt.get_action(), ArgAction::Count) {
            line.push(roman("..."))
        }
        line.push(roman(" "));
    }

    for arg in sub.get_positionals() {
        let (lhs, rhs) = option_markers_single(arg);
        line.push(roman(lhs));
        if let Some(value) = arg.get_value_names() {
            line.push(italic(value.join(" ")));
        } else {
            line.push(italic(arg.get_id().as_str()));
        }
        line.push(roman(rhs));
        line.push(roman(" "));
    }

    roff.text(line);
}

fn options(roff: &mut Roff, cmd: &clap::Command) {
    let items: Vec<_> = cmd.get_arguments().filter(|i| !i.is_hide_set()).collect();

    for pos in items.iter().filter(|a| a.is_positional()) {
        let mut header = vec![];
        let (lhs, rhs) = option_markers_single(pos);
        header.push(roman(lhs));
        if let Some(value) = pos.get_value_names() {
            header.push(italic(value.join(" ")));
        } else {
            header.push(italic(pos.get_id().as_str()));
        };
        header.push(roman(rhs));

        if let Some(defs) = option_default_values(pos) {
            header.push(roman(format!(" {defs}")));
        }

        let mut body = vec![];
        let mut arg_help_written = false;
        if let Some(help) = option_help(pos) {
            arg_help_written = true;
            let mut help = help.to_string();
            if !help.ends_with('.') {
                help.push('.');
            }
            body.push(roman(help));
        }

        roff.control("TP", []);
        roff.text(header);
        roff.text(body);

        if let Some(env) = option_environment(pos) {
            roff.control("RS", []);
            roff.text(env);
            roff.control("RE", []);
        }
        // If possible options are available
        if let Some((possible_values_text, with_help)) = get_possible_values(pos) {
            if arg_help_written {
                // It looks nice to have a separation between the help and the values
                roff.text([Inline::LineBreak]);
            }
            if with_help {
                roff.text([Inline::LineBreak, italic("Possible values:")]);

                // Need to indent twice to get it to look right, because .TP heading indents,
                // but that indent doesn't Carry over to the .IP for the
                // bullets. The standard shift size is 7 for terminal devices
                roff.control("RS", ["14"]);
                for line in possible_values_text {
                    roff.control("IP", ["\\(bu", "2"]);
                    roff.text([roman(line)]);
                }
                roff.control("RE", []);
            } else {
                let possible_value_text: Vec<Inline> = vec![
                    Inline::LineBreak,
                    roman("["),
                    italic("possible values: "),
                    roman(possible_values_text.join(", ")),
                    roman("]"),
                ];
                roff.text(possible_value_text);
            }
        }
    }

    for opt in items.iter().filter(|a| !a.is_positional()) {
        let mut header = match (opt.get_short(), opt.get_long()) {
            (Some(short), Some(long)) => {
                vec![short_option(short), roman(", "), long_option(long)]
            }
            (Some(short), None) => vec![short_option(short)],
            (None, Some(long)) => vec![long_option(long)],
            (None, None) => vec![],
        };

        if opt.get_action().takes_values() {
            if let Some(value) = &opt.get_value_names() {
                header.push(roman(" "));
                header.push(italic(value.join(" ")));
            }
        }

        if let Some(defs) = option_default_values(opt) {
            header.push(roman(" "));
            header.push(roman(defs));
        }

        let mut body = vec![];
        let mut arg_help_written = false;
        if let Some(help) = option_help(opt) {
            arg_help_written = true;
            let mut help = help.to_string();
            if !help.as_str().ends_with('.') {
                help.push('.');
            }

            body.push(roman(help));
        }

        roff.control("TP", []);
        roff.text(header);
        roff.text(body);

        if let Some((possible_values_text, with_help)) = get_possible_values(opt) {
            if arg_help_written {
                // It looks nice to have a separation between the help and the values
                roff.text([Inline::LineBreak, Inline::LineBreak]);
            }
            if with_help {
                roff.text([Inline::LineBreak, italic("Possible values:")]);

                // Need to indent twice to get it to look right, because .TP heading indents,
                // but that indent doesn't Carry over to the .IP for the
                // bullets. The standard shift size is 7 for terminal devices
                roff.control("RS", ["14"]);
                for line in possible_values_text {
                    roff.control("IP", ["\\(bu", "2"]);
                    roff.text([roman(line)]);
                }
                roff.control("RE", []);
            } else {
                let possible_value_text: Vec<Inline> = vec![
                    Inline::LineBreak,
                    roman("["),
                    italic("possible values: "),
                    roman(possible_values_text.join(", ")),
                    roman("]"),
                ];
                roff.text(possible_value_text);
            }
        }

        if let Some(env) = option_environment(opt) {
            roff.control("RS", []);
            roff.text(env);
            roff.control("RE", []);
        }
    }
}

fn option_markers(opt: &clap::Arg) -> (&'static str, &'static str) {
    markers(opt.is_required_set())
}

fn option_markers_single(opt: &clap::Arg) -> (&'static str, &'static str) {
    if opt.is_required_set() {
        ("", "")
    } else {
        markers(opt.is_required_set())
    }
}

fn markers(required: bool) -> (&'static str, &'static str) {
    if required {
        ("{", "}")
    } else {
        ("[", "]")
    }
}

fn short_option(opt: char) -> Inline {
    roman(format!("-{opt}"))
}

fn long_option(opt: &str) -> Inline {
    roman(format!("--{opt}"))
}

fn option_help(opt: &clap::Arg) -> Option<&clap::builder::StyledStr> {
    if !opt.is_hide_long_help_set() {
        let long_help = opt.get_long_help();
        if long_help.is_some() {
            return long_help;
        }
    }
    if !opt.is_hide_short_help_set() {
        return opt.get_help();
    }

    None
}

fn option_environment(opt: &clap::Arg) -> Option<Vec<Inline>> {
    if opt.is_hide_env_set() {
        return None;
    } else if let Some(env) = opt.get_env() {
        return Some(vec![
            roman("May also be specified with the "),
            bold(env.to_string_lossy().into_owned()),
            roman(" environment variable. "),
        ]);
    }

    None
}

fn option_default_values(opt: &clap::Arg) -> Option<String> {
    if opt.is_hide_default_value_set() || !opt.get_action().takes_values() {
        return None;
    } else if !opt.get_default_values().is_empty() {
        let values = opt
            .get_default_values()
            .iter()
            .map(|s| s.to_string_lossy())
            .collect::<Vec<_>>()
            .join(",");

        return Some(format!("[default: {values}]"));
    }

    None
}

fn get_possible_values(arg: &clap::Arg) -> Option<(Vec<String>, bool)> {
    let possibles = &arg.get_possible_values();
    let possibles: Vec<&clap::builder::PossibleValue> =
        possibles.iter().filter(|pos| !pos.is_hide_set()).collect();

    if !(possibles.is_empty() || arg.is_hide_possible_values_set()) {
        return Some(format_possible_values(&possibles));
    }
    None
}

fn format_possible_values(possibles: &Vec<&clap::builder::PossibleValue>) -> (Vec<String>, bool) {
    let mut lines = vec![];
    let with_help = possibles.iter().any(|p| p.get_help().is_some());
    if with_help {
        for value in possibles {
            let val_name = value.get_name();
            match value.get_help() {
                Some(help) => lines.push(format!(
                    "{val_name}: {help}{period}",
                    period = if help.to_string().ends_with('.') {
                        ""
                    } else {
                        "."
                    }
                )),
                None => lines.push(val_name.to_string()),
            }
        }
    } else {
        lines.append(&mut possibles.iter().map(|p| p.get_name().to_string()).collect());
    }
    (lines, with_help)
}
