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

//! Generate configuration for the postfix mail server.
//!
//! ## Transport maps (`transport_maps`)
//!
//! <http://www.postfix.org/postconf.5.html#transport_maps>
//!
//! ## Local recipient maps (`local_recipient_maps`)
//!
//! <http://www.postfix.org/postconf.5.html#local_recipient_maps>
//!
//! ## Relay domains (`relay_domains`)
//!
//! <http://www.postfix.org/postconf.5.html#relay_domains>

use std::{
    borrow::Cow,
    convert::TryInto,
    fs::OpenOptions,
    io::{BufWriter, Read, Seek, Write},
    path::{Path, PathBuf},
};

use crate::{errors::*, Configuration, Connection, DbVal, MailingList, PostPolicy};

/*
transport_maps =
    hash:/path-to-mailman/var/data/postfix_lmtp
local_recipient_maps =
    hash:/path-to-mailman/var/data/postfix_lmtp
relay_domains =
    hash:/path-to-mailman/var/data/postfix_domains
*/

/// Settings for generating postfix configuration.
///
/// See the struct methods for details.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PostfixConfiguration {
    /// The UNIX username under which the mailpot process who processed incoming
    /// mail is launched.
    pub user: Cow<'static, str>,
    /// The UNIX group under which the mailpot process who processed incoming
    /// mail is launched.
    pub group: Option<Cow<'static, str>>,
    /// The absolute path of the `mailpot` binary.
    pub binary_path: PathBuf,
    /// The maximum number of `mailpot` processes to launch. Default is `1`.
    #[serde(default)]
    pub process_limit: Option<u64>,
    /// The directory in which the map files are saved.
    /// Default is `data_path` from [`Configuration`](crate::Configuration).
    #[serde(default)]
    pub map_output_path: Option<PathBuf>,
    /// The name of the Postfix service name to use.
    /// Default is `mailpot`.
    ///
    /// A Postfix service is a daemon managed by the postfix process.
    /// Each entry in the `master.cf` configuration file defines a single
    /// service.
    ///
    /// The `master.cf` file is documented in [`master(5)`](https://www.postfix.org/master.5.html):
    /// <https://www.postfix.org/master.5.html>.
    #[serde(default)]
    pub transport_name: Option<Cow<'static, str>>,
}

impl Default for PostfixConfiguration {
    fn default() -> Self {
        Self {
            user: "user".into(),
            group: None,
            binary_path: Path::new("/usr/bin/mailpot").to_path_buf(),
            process_limit: None,
            map_output_path: None,
            transport_name: None,
        }
    }
}

impl PostfixConfiguration {
    /// Generate service line entry for Postfix's [`master.cf`](https://www.postfix.org/master.5.html) file.
    pub fn generate_master_cf_entry(&self, config: &Configuration, config_path: &Path) -> String {
        let transport_name = self.transport_name.as_deref().unwrap_or("mailpot");
        format!(
            "{transport_name} unix - n n - {process_limit} pipe
flags=RX user={username}{group_sep}{groupname} directory={{{data_dir}}} argv={{{binary_path}}} -c \
             {{{config_path}}} post",
            username = &self.user,
            group_sep = if self.group.is_none() { "" } else { ":" },
            groupname = self.group.as_deref().unwrap_or_default(),
            process_limit = self.process_limit.unwrap_or(1),
            binary_path = &self.binary_path.display(),
            config_path = &config_path.display(),
            data_dir = &config.data_path.display()
        )
    }

    /// Generate `transport_maps` and `local_recipient_maps` for Postfix.
    ///
    /// The output must be saved in a plain text file.
    /// To make Postfix be able to read them, the `postmap` application must be
    /// executed with the path to the map file as its sole argument.
    /// `postmap` is usually distributed along with the other Postfix binaries.
    pub fn generate_maps(
        &self,
        lists: &[(DbVal<MailingList>, Option<DbVal<PostPolicy>>)],
    ) -> String {
        let transport_name = self.transport_name.as_deref().unwrap_or("mailpot");
        let mut ret = String::new();
        ret.push_str("# Automatically generated by mailpot.\n");
        ret.push_str(
            "# Upon its creation and every time it is modified, postmap(1) must be called for the \
             changes to take effect:\n",
        );
        ret.push_str("# postmap /path/to/map_file\n\n");

        // [ref:TODO]: add custom addresses if PostPolicy is custom
        let calc_width = |list: &MailingList, policy: Option<&PostPolicy>| -> usize {
            let addr = list.address.len();
            match policy {
                None => 0,
                Some(PostPolicy { .. }) => addr + "+request".len(),
            }
        };

        let Some(width): Option<usize> = lists.iter().map(|(l, p)| calc_width(l, p.as_deref())).max() else {
            return ret;
        };

        for (list, policy) in lists {
            macro_rules! push_addr {
                ($addr:expr) => {{
                    let addr = &$addr;
                    ret.push_str(addr);
                    for _ in 0..(width - addr.len() + 5) {
                        ret.push(' ');
                    }
                    ret.push_str(transport_name);
                    ret.push_str(":\n");
                }};
            }

            match policy.as_deref() {
                None => log::debug!(
                    "Not generating postfix map entry for list {} because it has no post_policy \
                     set.",
                    list.id
                ),
                Some(PostPolicy { open: true, .. }) => {
                    push_addr!(list.address);
                    ret.push('\n');
                }
                Some(PostPolicy { .. }) => {
                    push_addr!(list.address);
                    push_addr!(list.subscription_mailto().address);
                    push_addr!(list.owner_mailto().address);
                    ret.push('\n');
                }
            }
        }

        // pop second of the last two newlines
        ret.pop();

        ret
    }

    /// Save service to Postfix's [`master.cf`](https://www.postfix.org/master.5.html) file.
    ///
    /// If you wish to do it manually, get the text output from
    /// [`PostfixConfiguration::generate_master_cf_entry`] and manually append it to the [`master.cf`](https://www.postfix.org/master.5.html) file.
    ///
    /// If `master_cf_path` is `None`, the location of the file is assumed to be
    /// `/etc/postfix/master.cf`.
    pub fn save_master_cf_entry(
        &self,
        config: &Configuration,
        config_path: &Path,
        master_cf_path: Option<&Path>,
    ) -> Result<()> {
        let new_entry = self.generate_master_cf_entry(config, config_path);
        let path = master_cf_path.unwrap_or_else(|| Path::new("/etc/postfix/master.cf"));

        // Create backup file.
        let path_bkp = path.with_extension("cf.bkp");
        std::fs::copy(path, &path_bkp).context(format!(
            "Could not create master.cf backup {}",
            path_bkp.display()
        ))?;
        log::info!(
            "Created backup of {} to {}.",
            path.display(),
            path_bkp.display()
        );

        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(false)
            .open(path)
            .context(format!("Could not open {}", path.display()))?;

        let mut previous_content = String::new();

        file.rewind()
            .context(format!("Could not access {}", path.display()))?;
        file.read_to_string(&mut previous_content)
            .context(format!("Could not access {}", path.display()))?;

        let original_size = previous_content.len();

        let lines = previous_content.lines().collect::<Vec<&str>>();
        let transport_name = self.transport_name.as_deref().unwrap_or("mailpot");

        if let Some(line) = lines.iter().find(|l| l.starts_with(transport_name)) {
            let pos = previous_content.find(line).ok_or_else(|| {
                Error::from(ErrorKind::Bug("Unepected logical error.".to_string()))
            })?;
            let end_needle = " argv=";
            let end_pos = previous_content[pos..]
                .find(end_needle)
                .and_then(|pos2| {
                    previous_content[(pos + pos2 + end_needle.len())..]
                        .find('\n')
                        .map(|p| p + pos + pos2 + end_needle.len())
                })
                .ok_or_else(|| {
                    Error::from(ErrorKind::Bug("Unepected logical error.".to_string()))
                })?;
            previous_content.replace_range(pos..end_pos, &new_entry);
        } else {
            previous_content.push_str(&new_entry);
            previous_content.push('\n');
        }

        file.rewind()?;
        if previous_content.len() < original_size {
            file.set_len(
                previous_content
                    .len()
                    .try_into()
                    .expect("Could not convert usize file size to u64"),
            )?;
        }
        let mut file = BufWriter::new(file);
        file.write_all(previous_content.as_bytes())
            .context(format!("Could not access {}", path.display()))?;
        file.flush()
            .context(format!("Could not access {}", path.display()))?;
        log::debug!("Saved new master.cf to {}.", path.display(),);

        Ok(())
    }

    /// Generate `transport_maps` and `local_recipient_maps` for Postfix.
    ///
    /// To succeed the user the command is running under must have write and
    /// read access to `postfix_data_directory` and the `postmap` binary
    /// must be discoverable in your `PATH` environment variable.
    ///
    /// `postmap` is usually distributed along with the other Postfix binaries.
    pub fn save_maps(&self, config: &Configuration) -> Result<()> {
        let db = Connection::open_db(config.clone())?;
        let Some(postmap) = find_binary_in_path("postmap") else {
            return Err(Error::from(ErrorKind::External(anyhow::Error::msg("Could not find postmap binary in PATH."))));
        };
        let lists = db.lists()?;
        let lists_post_policies = lists
            .into_iter()
            .map(|l| {
                let pk = l.pk;
                Ok((l, db.list_post_policy(pk)?))
            })
            .collect::<Result<Vec<(DbVal<MailingList>, Option<DbVal<PostPolicy>>)>>>()?;
        let content = self.generate_maps(&lists_post_policies);
        let path = self
            .map_output_path
            .as_deref()
            .unwrap_or(&config.data_path)
            .join("mailpot_postfix_map");
        let mut file = BufWriter::new(
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(true)
                .open(&path)
                .context(format!("Could not open {}", path.display()))?,
        );
        file.write_all(content.as_bytes())
            .context(format!("Could not write to {}", path.display()))?;
        file.flush()
            .context(format!("Could not write to {}", path.display()))?;

        let output = std::process::Command::new("sh")
            .arg("-c")
            .arg(&format!("{} {}", postmap.display(), path.display()))
            .output()
            .with_context(|| {
                format!(
                    "Could not execute `postmap` binary in path {}",
                    postmap.display()
                )
            })?;
        if !output.status.success() {
            use std::os::unix::process::ExitStatusExt;
            if let Some(code) = output.status.code() {
                return Err(Error::from(ErrorKind::External(anyhow::Error::msg(
                    format!(
                        "{} exited with {}.\nstderr was:\n---{}---\nstdout was\n---{}---\n",
                        code,
                        postmap.display(),
                        String::from_utf8_lossy(&output.stderr),
                        String::from_utf8_lossy(&output.stdout)
                    ),
                ))));
            } else if let Some(signum) = output.status.signal() {
                return Err(Error::from(ErrorKind::External(anyhow::Error::msg(
                    format!(
                        "{} was killed with signal {}.\nstderr was:\n---{}---\nstdout \
                         was\n---{}---\n",
                        signum,
                        postmap.display(),
                        String::from_utf8_lossy(&output.stderr),
                        String::from_utf8_lossy(&output.stdout)
                    ),
                ))));
            } else {
                return Err(Error::from(ErrorKind::External(anyhow::Error::msg(
                    format!(
                        "{} failed for unknown reason.\nstderr was:\n---{}---\nstdout \
                         was\n---{}---\n",
                        postmap.display(),
                        String::from_utf8_lossy(&output.stderr),
                        String::from_utf8_lossy(&output.stdout)
                    ),
                ))));
            }
        }

        Ok(())
    }
}

fn find_binary_in_path(binary_name: &str) -> Option<PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let full_path = dir.join(binary_name);
            if full_path.is_file() {
                Some(full_path)
            } else {
                None
            }
        })
    })
}

#[test]
fn test_postfix_generation() -> Result<()> {
    use tempfile::TempDir;

    use crate::*;

    crate::init_stderr_logging();

    fn get_smtp_conf() -> melib::smtp::SmtpServerConf {
        use melib::smtp::*;
        SmtpServerConf {
            hostname: "127.0.0.1".into(),
            port: 1025,
            envelope_from: "foo-chat@example.com".into(),
            auth: SmtpAuth::None,
            security: SmtpSecurity::None,
            extensions: Default::default(),
        }
    }

    let tmp_dir = TempDir::new()?;

    let db_path = tmp_dir.path().join("mpot.db");
    let config = Configuration {
        send_mail: SendMail::Smtp(get_smtp_conf()),
        db_path,
        data_path: tmp_dir.path().to_path_buf(),
        administrators: vec![],
    };
    let config_path = tmp_dir.path().join("conf.toml");
    {
        let mut conf = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&config_path)?;
        conf.write_all(config.to_toml().as_bytes())?;
        conf.flush()?;
    }

    let db = Connection::open_or_create_db(config)?.trusted();
    assert!(db.lists()?.is_empty());

    // Create three lists:
    //
    // - One without any policy, which should not show up in postfix maps.
    // - One with subscriptions disabled, which would only add the list address in
    //   postfix maps.
    // - One with subscriptions enabled, which should add all addresses (list,
    //   list+{un,}subscribe, etc).

    let first = db.create_list(MailingList {
        pk: 0,
        name: "first".into(),
        id: "first".into(),
        address: "first@example.com".into(),
        description: None,
        archive_url: None,
    })?;
    assert_eq!(first.pk(), 1);
    let second = db.create_list(MailingList {
        pk: 0,
        name: "second".into(),
        id: "second".into(),
        address: "second@example.com".into(),
        description: None,
        archive_url: None,
    })?;
    assert_eq!(second.pk(), 2);
    let post_policy = db.set_list_post_policy(PostPolicy {
        pk: 0,
        list: second.pk(),
        announce_only: false,
        subscription_only: false,
        approval_needed: false,
        open: true,
        custom: false,
    })?;

    assert_eq!(post_policy.pk(), 1);
    let third = db.create_list(MailingList {
        pk: 0,
        name: "third".into(),
        id: "third".into(),
        address: "third@example.com".into(),
        description: None,
        archive_url: None,
    })?;
    assert_eq!(third.pk(), 3);
    let post_policy = db.set_list_post_policy(PostPolicy {
        pk: 0,
        list: third.pk(),
        announce_only: false,
        subscription_only: false,
        approval_needed: true,
        open: false,
        custom: false,
    })?;

    assert_eq!(post_policy.pk(), 2);

    let mut postfix_conf = PostfixConfiguration::default();

    let expected_mastercf_entry = format!(
        "mailpot unix - n n - 1 pipe
flags=RX user={} directory={{{}}} argv={{/usr/bin/mailpot}} -c {{{}}} post\n",
        &postfix_conf.user,
        tmp_dir.path().display(),
        config_path.display()
    );
    assert_eq!(
        expected_mastercf_entry.trim_end(),
        postfix_conf.generate_master_cf_entry(db.conf(), &config_path)
    );

    let lists = db.lists()?;
    let lists_post_policies = lists
        .into_iter()
        .map(|l| {
            let pk = l.pk;
            Ok((l, db.list_post_policy(pk)?))
        })
        .collect::<Result<Vec<(DbVal<MailingList>, Option<DbVal<PostPolicy>>)>>>()?;
    let maps = postfix_conf.generate_maps(&lists_post_policies);

    let expected = "second@example.com             mailpot:

third@example.com              mailpot:
third+request@example.com      mailpot:
third+owner@example.com        mailpot:
";
    assert!(
        maps.ends_with(expected),
        "maps has unexpected contents: has\n{:?}\nand should have ended with\n{:?}",
        maps,
        expected
    );

    let master_edit_value = r#"#
# Postfix master process configuration file.  For details on the format
# of the file, see the master(5) manual page (command: "man 5 master" or
# on-line: http://www.postfix.org/master.5.html).
#
# Do not forget to execute "postfix reload" after editing this file.
#
# ==========================================================================
# service type  private unpriv  chroot  wakeup  maxproc command + args
#               (yes)   (yes)   (no)    (never) (100)
# ==========================================================================
smtp      inet  n       -       y       -       -       smtpd
pickup    unix  n       -       y       60      1       pickup
cleanup   unix  n       -       y       -       0       cleanup
qmgr      unix  n       -       n       300     1       qmgr
#qmgr     unix  n       -       n       300     1       oqmgr
tlsmgr    unix  -       -       y       1000?   1       tlsmgr
rewrite   unix  -       -       y       -       -       trivial-rewrite
bounce    unix  -       -       y       -       0       bounce
defer     unix  -       -       y       -       0       bounce
trace     unix  -       -       y       -       0       bounce
verify    unix  -       -       y       -       1       verify
flush     unix  n       -       y       1000?   0       flush
proxymap  unix  -       -       n       -       -       proxymap
proxywrite unix -       -       n       -       1       proxymap
smtp      unix  -       -       y       -       -       smtp
relay     unix  -       -       y       -       -       smtp
        -o syslog_name=postfix/$service_name
showq     unix  n       -       y       -       -       showq
error     unix  -       -       y       -       -       error
retry     unix  -       -       y       -       -       error
discard   unix  -       -       y       -       -       discard
local     unix  -       n       n       -       -       local
virtual   unix  -       n       n       -       -       virtual
lmtp      unix  -       -       y       -       -       lmtp
anvil     unix  -       -       y       -       1       anvil
scache    unix  -       -       y       -       1       scache
postlog   unix-dgram n  -       n       -       1       postlogd
maildrop  unix  -       n       n       -       -       pipe
  flags=DRXhu user=vmail argv=/usr/bin/maildrop -d ${recipient}
uucp      unix  -       n       n       -       -       pipe
  flags=Fqhu user=uucp argv=uux -r -n -z -a$sender - $nexthop!rmail ($recipient)
#
# Other external delivery methods.
#
ifmail    unix  -       n       n       -       -       pipe
  flags=F user=ftn argv=/usr/lib/ifmail/ifmail -r $nexthop ($recipient)
bsmtp     unix  -       n       n       -       -       pipe
  flags=Fq. user=bsmtp argv=/usr/lib/bsmtp/bsmtp -t$nexthop -f$sender $recipient
scalemail-backend unix -       n       n       -       2       pipe
  flags=R user=scalemail argv=/usr/lib/scalemail/bin/scalemail-store ${nexthop} ${user} ${extension}
mailman   unix  -       n       n       -       -       pipe
  flags=FRX user=list argv=/usr/lib/mailman/bin/postfix-to-mailman.py ${nexthop} ${user}
"#;

    let path = tmp_dir.path().join("master.cf");
    {
        let mut mastercf = OpenOptions::new().write(true).create(true).open(&path)?;
        mastercf.write_all(master_edit_value.as_bytes())?;
        mastercf.flush()?;
    }
    postfix_conf.save_master_cf_entry(db.conf(), &config_path, Some(&path))?;
    let mut first = String::new();
    {
        let mut mastercf = OpenOptions::new()
            .write(false)
            .read(true)
            .create(false)
            .open(&path)?;
        mastercf.read_to_string(&mut first)?;
    }
    assert!(
        first.ends_with(&expected_mastercf_entry),
        "edited master.cf has unexpected contents: has\n{:?}\nand should have ended with\n{:?}",
        first,
        expected_mastercf_entry
    );

    // test that a smaller entry can be successfully replaced

    postfix_conf.user = "nobody".into();
    postfix_conf.save_master_cf_entry(db.conf(), &config_path, Some(&path))?;
    let mut second = String::new();
    {
        let mut mastercf = OpenOptions::new()
            .write(false)
            .read(true)
            .create(false)
            .open(&path)?;
        mastercf.read_to_string(&mut second)?;
    }
    let expected_mastercf_entry = format!(
        "mailpot unix - n n - 1 pipe
flags=RX user=nobody directory={{{}}} argv={{/usr/bin/mailpot}} -c {{{}}} post\n",
        tmp_dir.path().display(),
        config_path.display()
    );
    assert!(
        second.ends_with(&expected_mastercf_entry),
        "doubly edited master.cf has unexpected contents: has\n{:?}\nand should have ended \
         with\n{:?}",
        second,
        expected_mastercf_entry
    );
    // test that a larger entry can be successfully replaced
    postfix_conf.user = "hackerman".into();
    postfix_conf.save_master_cf_entry(db.conf(), &config_path, Some(&path))?;
    let mut third = String::new();
    {
        let mut mastercf = OpenOptions::new()
            .write(false)
            .read(true)
            .create(false)
            .open(&path)?;
        mastercf.read_to_string(&mut third)?;
    }
    let expected_mastercf_entry = format!(
        "mailpot unix - n n - 1 pipe
flags=RX user=hackerman directory={{{}}} argv={{/usr/bin/mailpot}} -c {{{}}} post\n",
        tmp_dir.path().display(),
        config_path.display(),
    );
    assert!(
        third.ends_with(&expected_mastercf_entry),
        "triply edited master.cf has unexpected contents: has\n{:?}\nand should have ended \
         with\n{:?}",
        third,
        expected_mastercf_entry
    );

    // test that if groupname is given it is rendered correctly.
    postfix_conf.group = Some("nobody".into());
    postfix_conf.save_master_cf_entry(db.conf(), &config_path, Some(&path))?;
    let mut fourth = String::new();
    {
        let mut mastercf = OpenOptions::new()
            .write(false)
            .read(true)
            .create(false)
            .open(&path)?;
        mastercf.read_to_string(&mut fourth)?;
    }
    let expected_mastercf_entry = format!(
        "mailpot unix - n n - 1 pipe
flags=RX user=hackerman:nobody directory={{{}}} argv={{/usr/bin/mailpot}} -c {{{}}} post\n",
        tmp_dir.path().display(),
        config_path.display(),
    );
    assert!(
        fourth.ends_with(&expected_mastercf_entry),
        "fourthly edited master.cf has unexpected contents: has\n{:?}\nand should have ended \
         with\n{:?}",
        fourth,
        expected_mastercf_entry
    );

    Ok(())
}
