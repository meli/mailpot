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

use mailpot::{
    chrono,
    melib::{self, Envelope},
    models::{Account, DbVal, ListSubscription, MailingList},
    rusqlite, Connection, Result,
};

pub fn datetime_header_value_lint(db: &mut Connection, dry_run: bool) -> Result<()> {
    let mut col = vec![];
    {
        let mut stmt = db.connection.prepare("SELECT * FROM post ORDER BY pk")?;
        let iter = stmt.query_map([], |row| {
            let pk: i64 = row.get("pk")?;
            let date_s: String = row.get("datetime")?;
            match melib::datetime::rfc822_to_timestamp(date_s.trim()) {
                Err(_) | Ok(0) => {
                    let mut timestamp: i64 = row.get("timestamp")?;
                    let created: i64 = row.get("created")?;
                    if timestamp == 0 {
                        timestamp = created;
                    }
                    timestamp = std::cmp::min(timestamp, created);
                    let timestamp = if timestamp <= 0 {
                        None
                    } else {
                        // safe because we checked it's not negative or zero above.
                        Some(timestamp as u64)
                    };
                    let message: Vec<u8> = row.get("message")?;
                    Ok(Some((pk, date_s, message, timestamp)))
                }
                Ok(_) => Ok(None),
            }
        })?;

        for entry in iter {
            if let Some(s) = entry? {
                col.push(s);
            }
        }
    }
    let mut failures = 0;
    let tx = if dry_run {
        None
    } else {
        Some(db.connection.transaction()?)
    };
    if col.is_empty() {
        println!("datetime_header_value: ok");
    } else {
        println!("datetime_header_value: found {} entries", col.len());
        println!("pk\tDate value\tshould be");
        for (pk, val, message, timestamp) in col {
            let correct = if let Ok(v) =
                chrono::DateTime::<chrono::FixedOffset>::parse_from_rfc3339(&val)
            {
                v.to_rfc2822()
            } else if let Some(v) = timestamp.map(|t| {
                melib::datetime::timestamp_to_string(t, Some(melib::datetime::RFC822_DATE), true)
            }) {
                v
            } else if let Ok(v) =
                Envelope::from_bytes(&message, None).map(|env| env.date_as_str().to_string())
            {
                v
            } else {
                failures += 1;
                println!("{pk}\t{val}\tCould not find any valid date value in the post metadata!");
                continue;
            };
            println!("{pk}\t{val}\t{correct}");
            if let Some(tx) = tx.as_ref() {
                tx.execute(
                    "UPDATE post SET datetime = ? WHERE pk = ?",
                    rusqlite::params![&correct, pk],
                )?;
            }
        }
    }
    if let Some(tx) = tx {
        tx.commit()?;
    }
    if failures > 0 {
        println!(
            "datetime_header_value: {failures} failure{}",
            if failures == 1 { "" } else { "s" }
        );
    }
    Ok(())
}

pub fn remove_empty_accounts_lint(db: &mut Connection, dry_run: bool) -> Result<()> {
    let mut col = vec![];
    {
        let mut stmt = db.connection.prepare(
            "SELECT * FROM account WHERE NOT EXISTS (SELECT 1 FROM subscription AS s WHERE \
             s.address = address) ORDER BY pk",
        )?;
        let iter = stmt.query_map([], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                Account {
                    pk,
                    name: row.get("name")?,
                    address: row.get("address")?,
                    public_key: row.get("public_key")?,
                    password: row.get("password")?,
                    enabled: row.get("enabled")?,
                },
                pk,
            ))
        })?;

        for entry in iter {
            let entry = entry?;
            col.push(entry);
        }
    }
    if col.is_empty() {
        println!("remove_empty_accounts: ok");
    } else {
        let tx = if dry_run {
            None
        } else {
            Some(db.connection.transaction()?)
        };
        println!("remove_empty_accounts: found {} entries", col.len());
        println!("pk\tAddress");
        for DbVal(Account { pk, address, .. }, _) in &col {
            println!("{pk}\t{address}");
        }
        if let Some(tx) = tx {
            for DbVal(_, pk) in col {
                tx.execute("DELETE FROM account WHERE pk = ?", [pk])?;
            }
            tx.commit()?;
        }
    }
    Ok(())
}

pub fn remove_accepted_subscription_requests_lint(
    db: &mut Connection,
    dry_run: bool,
) -> Result<()> {
    let mut col = vec![];
    {
        let mut stmt = db.connection.prepare(
            "SELECT * FROM candidate_subscription WHERE accepted IS NOT NULL ORDER BY pk",
        )?;
        let iter = stmt.query_map([], |row| {
            let pk = row.get("pk")?;
            Ok(DbVal(
                ListSubscription {
                    pk,
                    list: row.get("list")?,
                    address: row.get("address")?,
                    account: row.get("account")?,
                    name: row.get("name")?,
                    digest: row.get("digest")?,
                    enabled: row.get("enabled")?,
                    verified: row.get("verified")?,
                    hide_address: row.get("hide_address")?,
                    receive_duplicates: row.get("receive_duplicates")?,
                    receive_own_posts: row.get("receive_own_posts")?,
                    receive_confirmation: row.get("receive_confirmation")?,
                },
                pk,
            ))
        })?;

        for entry in iter {
            let entry = entry?;
            col.push(entry);
        }
    }
    if col.is_empty() {
        println!("remove_accepted_subscription_requests: ok");
    } else {
        let tx = if dry_run {
            None
        } else {
            Some(db.connection.transaction()?)
        };
        println!(
            "remove_accepted_subscription_requests: found {} entries",
            col.len()
        );
        println!("pk\tAddress");
        for DbVal(ListSubscription { pk, address, .. }, _) in &col {
            println!("{pk}\t{address}");
        }
        if let Some(tx) = tx {
            for DbVal(_, pk) in col {
                tx.execute("DELETE FROM candidate_subscription WHERE pk = ?", [pk])?;
            }
            tx.commit()?;
        }
    }
    Ok(())
}

pub fn warn_list_no_owner_lint(db: &mut Connection, _: bool) -> Result<()> {
    let mut stmt = db.connection.prepare(
        "SELECT * FROM list WHERE NOT EXISTS (SELECT 1 FROM owner AS o WHERE o.list = pk) ORDER \
         BY pk",
    )?;
    let iter = stmt.query_map([], |row| {
        let pk = row.get("pk")?;
        Ok(DbVal(
            MailingList {
                pk,
                name: row.get("name")?,
                id: row.get("id")?,
                address: row.get("address")?,
                description: row.get("description")?,
                topics: vec![],
                archive_url: row.get("archive_url")?,
            },
            pk,
        ))
    })?;

    let mut col = vec![];
    for entry in iter {
        let entry = entry?;
        col.push(entry);
    }
    if col.is_empty() {
        println!("warn_list_no_owner: ok");
    } else {
        println!("warn_list_no_owner: found {} entries", col.len());
        println!("pk\tName");
        for DbVal(MailingList { pk, name, .. }, _) in col {
            println!("{pk}\t{name}");
        }
    }
    Ok(())
}
