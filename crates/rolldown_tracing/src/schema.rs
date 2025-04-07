// @generated automatically by Diesel CLI.

diesel::table! {
    Event (id) {
        is_event -> Bool,
        id -> Integer,
        created_at -> Timestamp,
        build_id -> Integer,
    }
}

diesel::table! {
    build (id) {
        is_build -> Bool,
        id -> Integer,
    }
}

diesel::table! {
    hook_call_meta (id) {
        is_hook_call_meta -> Bool,
        id -> Integer,
        plugin_name -> Text,
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
        is_plugin_hook_transform_end -> Bool,
        id -> Integer,
        event_id -> Integer,
        hook_call_meta_id -> Integer,
        transformed_source -> Text,
        module_id -> Text,
    }
}

diesel::table! {
    hook_transform_start (id) {
        is_hook_transform_start -> Bool,
        id -> Integer,
        event_id -> Integer,
        hook_call_meta_id -> Integer,
        source -> Text,
        module_id -> Text,
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
    build,
    hook_call_meta,
    hook_transform_call,
    hook_transform_end,
    hook_transform_start,
);
