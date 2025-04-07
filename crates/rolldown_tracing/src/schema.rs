// @generated automatically by Diesel CLI.

diesel::table! {
    Event (id) {
        created_at -> Timestamp,
        build_id -> Integer,
        is_event -> Bool,
        id -> Integer,
    }
}

diesel::table! {
    _prisma_migrations (id) {
        id -> Text,
        checksum -> Text,
        finished_at -> Nullable<Timestamp>,
        migration_name -> Text,
        logs -> Nullable<Text>,
        rolled_back_at -> Nullable<Timestamp>,
        started_at -> Timestamp,
        applied_steps_count -> Integer,
    }
}

diesel::table! {
    build (id) {
        created_at -> Timestamp,
        is_build -> Bool,
        id -> Integer,
    }
}

diesel::table! {
    hook_call_meta (id) {
        plugin_name -> Text,
        is_hook_call_meta -> Bool,
        id -> Integer,
    }
}

diesel::table! {
    hook_transform_call (id) {
        is_hook_transform_call -> Bool,
        id -> Integer,
        event_id -> Integer,
        hook_call_meta_id -> Integer,
        plugin_hook_transform_start_id -> Integer,
        plugin_hook_transform_end_id -> Integer,
    }
}

diesel::table! {
    hook_transform_end (id) {
        transformed_source -> Text,
        module_id -> Text,
        is_plugin_hook_transform_end -> Bool,
        id -> Integer,
        event_id -> Integer,
        hook_call_meta_id -> Integer,
    }
}

diesel::table! {
    hook_transform_start (id) {
        source -> Text,
        module_id -> Text,
        is_hook_transform_start -> Bool,
        id -> Integer,
        event_id -> Integer,
        hook_call_meta_id -> Integer,
    }
}

diesel::joinable!(Event -> build (build_id));
diesel::joinable!(hook_transform_call -> Event (event_id));
diesel::joinable!(hook_transform_call -> hook_call_meta (hook_call_meta_id));
diesel::joinable!(hook_transform_call -> hook_transform_end (plugin_hook_transform_end_id));
diesel::joinable!(hook_transform_call -> hook_transform_start (plugin_hook_transform_start_id));
diesel::joinable!(hook_transform_end -> Event (event_id));
diesel::joinable!(hook_transform_end -> hook_call_meta (hook_call_meta_id));
diesel::joinable!(hook_transform_start -> Event (event_id));
diesel::joinable!(hook_transform_start -> hook_call_meta (hook_call_meta_id));

diesel::allow_tables_to_appear_in_same_query!(
    Event,
    _prisma_migrations,
    build,
    hook_call_meta,
    hook_transform_call,
    hook_transform_end,
    hook_transform_start,
);
