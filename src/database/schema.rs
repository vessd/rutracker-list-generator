table! {
    forums (id) {
        id -> SmallInt,
        name -> Text,
        tor_count -> Integer,
        tor_size_bytes -> Double,
        topic_id -> Integer,
    }
}

table! {
    keeper_torrents (keeper, topic_id) {
        keeper -> Text,
        topic_id -> Integer,
    }
}

table! {
    local_torrents (hash, url) {
        hash -> Text,
        status -> SmallInt,
        url -> Text,
    }
}

table! {
    topics (id) {
        id -> Integer,
        title -> Text,
        author -> Text,
    }
}

table! {
    torrents (topic_id) {
        topic_id -> Integer,
        forum_id -> SmallInt,
        poster_id -> Integer,
        title -> Text,
        hash -> Text,
        size -> Double,
        reg_time -> Timestamp,
        status -> SmallInt,
        seeders -> SmallInt,
    }
}

joinable!(forums -> topics (topic_id));
joinable!(keeper_torrents -> torrents (topic_id));
joinable!(torrents -> forums (forum_id));

allow_tables_to_appear_in_same_query!(forums, keeper_torrents, local_torrents, topics, torrents,);
