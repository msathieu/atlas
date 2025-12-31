// @generated automatically by Diesel CLI.

diesel::table! {
    chats (id) {
        id -> Uuid,
        uid -> Uuid,
        summary -> Varchar,
        start_timestamp -> Timestamp,
        end_timestamp -> Timestamp,
        error_count -> Int4,
        consecutive_llm_messages -> Int4,
        max_consecutive_llm_messages -> Int4,
    }
}

diesel::table! {
    commands (id) {
        id -> Uuid,
        device -> Uuid,
        uid -> Nullable<Uuid>,
        code -> Varchar,
        completed -> Bool,
        status_code -> Int4,
        stdout -> Varchar,
        stderr -> Varchar,
        requested_timestamp -> Timestamp,
        completed_timestamp -> Nullable<Timestamp>,
        origin -> Int4,
        origin_id -> Varchar,
    }
}

diesel::table! {
    device_info (device, info_type) {
        device -> Uuid,
        info_type -> Varchar,
        success -> Bool,
        data -> Varchar,
        updated_timestamp -> Timestamp,
    }
}

diesel::table! {
    devices (id) {
        id -> Uuid,
        arch -> Varchar,
        os -> Varchar,
        version -> Varchar,
        pubkey -> Bytea,
        approved -> Bool,
        hostname -> Varchar,
        first_connection -> Timestamp,
        last_connection -> Timestamp,
        ip -> Varchar,
        agent_hash -> Bytea,
    }
}

diesel::table! {
    messages (chat, sequence) {
        chat -> Uuid,
        sequence -> Int4,
        content -> Varchar,
        sent_on -> Timestamp,
        user_message -> Bool,
        generation_id -> Nullable<Varchar>,
        finished -> Bool,
        reasoning -> Nullable<Varchar>,
        input_tokens -> Int4,
        output_tokens -> Int4,
        model -> Varchar,
    }
}

diesel::table! {
    settings (name) {
        name -> Varchar,
        value -> Varchar,
    }
}

diesel::table! {
    tool_calls (chat, sequence, call_id) {
        chat -> Uuid,
        sequence -> Int4,
        call_id -> Varchar,
        tool -> Varchar,
        request -> Varchar,
        response -> Nullable<Varchar>,
        user_consented -> Nullable<Bool>,
        error_count -> Int4,
    }
}

diesel::table! {
    users (id) {
        id -> Uuid,
        username -> Varchar,
        password -> Varchar,
        jwt_counter -> Int4,
        admin -> Bool,
    }
}

diesel::joinable!(chats -> users (uid));
diesel::joinable!(commands -> devices (device));
diesel::joinable!(commands -> users (uid));
diesel::joinable!(device_info -> devices (device));
diesel::joinable!(messages -> chats (chat));

diesel::allow_tables_to_appear_in_same_query!(chats, commands, device_info, devices, messages, settings, tool_calls, users,);
