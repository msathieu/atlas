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
        chat_type -> Int4,
        refresh_consecutive_llm_messages -> Nullable<Timestamp>,
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
        device_info_type -> Nullable<Varchar>,
        chat_id -> Nullable<Uuid>,
        chat_sequence -> Nullable<Int4>,
        chat_tool_call_id -> Nullable<Varchar>,
        initiated -> Bool,
    }
}

diesel::table! {
    containers (id) {
        id -> Uuid,
        uid -> Uuid,
        origin -> Int4,
        state -> Int4,
        created_at -> Timestamp,
        image -> Varchar,
        requested_state -> Int4,
    }
}

diesel::table! {
    development_environments (id) {
        id -> Uuid,
        repo -> Varchar,
        branch -> Nullable<Varchar>,
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
        container_id -> Nullable<Uuid>,
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
        provider -> Varchar,
        model -> Varchar,
        compacted -> Bool,
    }
}

diesel::table! {
    passkeys (id) {
        id -> Bytea,
        uid -> Uuid,
        passkey -> Varchar,
        created_at -> Timestamp,
        last_used -> Nullable<Timestamp>,
        name -> Varchar,
    }
}

diesel::table! {
    preapproved_code (id) {
        id -> Uuid,
        chat -> Uuid,
        code -> Varchar,
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
        initiated -> Bool,
        provider_id -> Nullable<Varchar>,
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
diesel::joinable!(containers -> users (uid));
diesel::joinable!(development_environments -> containers (id));
diesel::joinable!(device_info -> devices (device));
diesel::joinable!(devices -> containers (container_id));
diesel::joinable!(messages -> chats (chat));
diesel::joinable!(passkeys -> users (uid));
diesel::joinable!(preapproved_code -> chats (chat));

diesel::allow_tables_to_appear_in_same_query!(
    chats,
    commands,
    containers,
    development_environments,
    device_info,
    devices,
    messages,
    passkeys,
    preapproved_code,
    settings,
    tool_calls,
    users,
);
