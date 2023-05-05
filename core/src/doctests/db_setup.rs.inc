# use mailpot::{*, models::*};
# use melib::smtp::{SmtpServerConf, SmtpAuth, SmtpSecurity};
#
# use tempfile::TempDir;
#
# let tmp_dir = TempDir::new()?;
# let db_path = tmp_dir.path().join("mpot.db");
# let data_path = tmp_dir.path().to_path_buf();
# let config = Configuration {
#   send_mail: mailpot::SendMail::Smtp(
#     SmtpServerConf {
#       hostname: "127.0.0.1".into(),
#       port: 25,
#       envelope_from: "foo-chat@example.com".into(),
#       auth: SmtpAuth::None,
#       security: SmtpSecurity::None,
#       extensions: Default::default(),
#     }
#   ),
#   db_path,
#   data_path,
#   administrators: vec![],
# };
# let db = Connection::open_or_create_db(config)?.trusted();
# let list = db
#     .create_list(MailingList {
#         pk: 5,
#         name: "foobar chat".into(),
#         id: "foo-chat".into(),
#         address: "foo-chat@example.com".into(),
#         description: Some("Hello world, from foo-chat list".into()),
#         archive_url: Some("https://lists.example.com".into()),
#     })
#     .unwrap();
# let sub_policy = SubscriptionPolicy {
#    pk: 1,
#    list: 5,
#    send_confirmation: true,
#    open: false,
#    manual: false,
#    request: true,
#    custom: false,
# };
# let post_policy = PostPolicy {
# pk: 1,
# list: 5,
#    announce_only: false,
#    subscription_only: false,
#    approval_needed: false,
#    open: true,
#    custom: false,
# };
