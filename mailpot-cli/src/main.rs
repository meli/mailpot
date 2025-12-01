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

#![allow(clippy::result_large_err)]

use std::path::PathBuf;

use clap::{CommandFactory, Parser};
use mailpot::{melib::smtp, Configuration, Connection, Context, Result};
use mailpot_cli::{args::*, commands::*};

fn run_app(
    config: Option<PathBuf>,
    cmd: Command,
    debug: bool,
    quiet: bool,
    verbose: u8,
) -> Result<()> {
    if let Command::SampleConfig { with_smtp } = cmd {
        let mut new = Configuration::new("/path/to/sqlite.db");
        new.administrators.push("admin@example.com".to_string());
        if with_smtp {
            new.send_mail = mailpot::SendMail::Smtp(smtp::SmtpServerConf {
                hostname: "mail.example.com".to_string(),
                port: 587,
                envelope_from: "".to_string(),
                auth: smtp::SmtpAuth::Auto {
                    username: "user".to_string(),
                    password: smtp::Password::Raw("hunter2".to_string()),
                    auth_type: smtp::SmtpAuthType::default(),
                    require_auth: true,
                },
                security: smtp::SmtpSecurity::StartTLS {
                    danger_accept_invalid_certs: false,
                },
                extensions: Default::default(),
            });
        }
        println!("{}", new.to_toml());
        return Ok(());
    };
    let config_path = if let Some(path) = config.as_deref() {
        path
    } else {
        let mut opt = Opt::command();
        opt.error(
            clap::error::ErrorKind::MissingRequiredArgument,
            "--config is required for mailing list operations",
        )
        .exit();
    };

    let config = Configuration::from_file(config_path).with_context(|| {
        format!(
            "Could not read configuration file from path: {}",
            config_path.display()
        )
    })?;

    use Command::*;
    let mut db = Connection::open_or_create_db(config)
        .context("Could not open database connection with this configuration")?
        .trusted();
    match cmd {
        SampleConfig { .. } => {}
        DumpDatabase => {
            dump_database(&mut db).context("Could not dump database.")?;
        }
        ListLists => {
            list_lists(&mut db).context("Could not retrieve mailing lists.")?;
        }
        List { list_id, cmd } => {
            list(&mut db, &list_id, cmd, quiet).map_err(|err| {
                err.chain_err(|| {
                    mailpot::Error::from(format!("Could not perform list command for {list_id}."))
                })
            })?;
        }
        CreateList {
            name,
            id,
            address,
            description,
            archive_url,
        } => {
            create_list(&mut db, name, id, address, description, archive_url, quiet)
                .context("Could not create list.")?;
        }
        Post { dry_run } => {
            post(&mut db, dry_run, debug).context("Could not process post.")?;
        }
        FlushQueue { dry_run } => {
            flush_queue(&mut db, dry_run, verbose, debug).with_context(|| {
                format!("Could not flush queue {}.", mailpot::queue::Queue::Out)
            })?;
        }
        Queue { queue, cmd } => {
            queue_(&mut db, queue, cmd, quiet)
                .with_context(|| format!("Could not perform queue command for queue `{queue}`."))?;
        }
        ImportMaildir {
            list_id,
            maildir_path,
        } => {
            import_maildir(
                &mut db,
                &list_id,
                maildir_path.clone(),
                quiet,
                debug,
                verbose,
            )
            .with_context(|| {
                format!(
                    "Could not import maildir path {} to list `{list_id}`.",
                    maildir_path.display(),
                )
            })?;
        }
        UpdatePostfixConfig { master_cf, config } => {
            update_postfix_config(config_path, &mut db, master_cf, config)
                .context("Could not update postfix configuration.")?;
        }
        PrintPostfixConfig { config } => {
            print_postfix_config(config_path, &mut db, config)
                .context("Could not print postfix configuration.")?;
        }
        Accounts => {
            accounts(&mut db, quiet).context("Could not retrieve accounts.")?;
        }
        AccountInfo { address } => {
            account_info(&mut db, &address, quiet).with_context(|| {
                format!("Could not retrieve account info for address {address}.")
            })?;
        }
        AddAccount {
            address,
            password,
            name,
            public_key,
            enabled,
        } => {
            add_account(&mut db, address, password, name, public_key, enabled)
                .context("Could not add account.")?;
        }
        RemoveAccount { address } => {
            remove_account(&mut db, &address, quiet)
                .with_context(|| format!("Could not remove account with address {address}."))?;
        }
        UpdateAccount {
            address,
            password,
            name,
            public_key,
            enabled,
        } => {
            update_account(&mut db, address, password, name, public_key, enabled)
                .context("Could not update account.")?;
        }
        Repair {
            fix,
            all,
            datetime_header_value,
            remove_empty_accounts,
            remove_accepted_subscription_requests,
            warn_list_no_owner,
        } => {
            repair(
                &mut db,
                fix,
                all,
                datetime_header_value,
                remove_empty_accounts,
                remove_accepted_subscription_requests,
                warn_list_no_owner,
            )
            .context("Could not perform database repair.")?;
        }
    }

    Ok(())
}

fn main() -> std::result::Result<(), i32> {
    let opt = Opt::parse();
    stderrlog::new()
        .module(module_path!())
        .module("mailpot")
        .quiet(opt.quiet)
        .verbosity(opt.verbose as usize)
        .timestamp(opt.ts.unwrap_or(stderrlog::Timestamp::Off))
        .init()
        .unwrap();
    if opt.debug {
        println!("DEBUG: {:?}", &opt);
    }
    let Opt {
        config,
        cmd,
        debug,
        quiet,
        verbose,
        ..
    } = opt;
    if let Err(err) = run_app(config, cmd, debug, quiet, verbose) {
        print!("{}", err.display_chain());
        std::process::exit(-1);
    }
    Ok(())
}
