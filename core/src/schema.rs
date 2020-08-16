table! {
    list_owner (pk) {
        pk -> Integer,
        list -> Integer,
        address -> Text,
        name -> Nullable<Text>,
    }
}

table! {
    mailing_lists (pk) {
        pk -> Integer,
        name -> Text,
        id -> Text,
        address -> Text,
        archive_url -> Nullable<Text>,
        description -> Nullable<Text>,
    }
}

table! {
    membership (list, address) {
        list -> Integer,
        address -> Text,
        name -> Nullable<Text>,
        digest -> Bool,
        hide_address -> Bool,
        receive_duplicates -> Bool,
        receive_own_posts -> Bool,
        receive_confirmation -> Bool,
    }
}

table! {
    post (pk) {
        pk -> Integer,
        list -> Integer,
        address -> Text,
        message_id -> Text,
        message -> Binary,
    }
}

table! {
    post_event (pk) {
        pk -> Integer,
        post -> Integer,
        date -> Integer,
        kind -> Text,
        content -> Text,
    }
}

table! {
    post_policy (pk) {
        pk -> Integer,
        list -> Integer,
        announce_only -> Bool,
        subscriber_only -> Bool,
        approval_needed -> Bool,
    }
}

joinable!(list_owner -> mailing_lists (list));
joinable!(membership -> mailing_lists (list));
joinable!(post_event -> post (post));
joinable!(post_policy -> mailing_lists (list));

allow_tables_to_appear_in_same_query!(
    list_owner,
    mailing_lists,
    membership,
    post,
    post_event,
    post_policy,
);
