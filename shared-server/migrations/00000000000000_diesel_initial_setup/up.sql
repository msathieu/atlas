CREATE TABLE settings(
    name varchar PRIMARY KEY,
    value varchar NOT NULL
);
CREATE TABLE users(
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    username varchar NOT NULL UNIQUE,
    password varchar NOT NULL,
    jwt_counter integer NOT NULL,
    admin boolean NOT NULL,
    enabled boolean NOT NULL,
    password_enabled boolean NOT NULL,
    last_login timestamp
);
CREATE TABLE chats(
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    uid uuid NOT NULL REFERENCES users ON DELETE CASCADE,
    summary varchar NOT NULL,
    start_timestamp timestamp NOT NULL,
    end_timestamp timestamp NOT NULL,
    error_count integer NOT NULL,
    consecutive_llm_messages integer NOT NULL,
    max_consecutive_llm_messages integer NOT NULL,
    chat_type integer NOT NULL,
    refresh_consecutive_llm_messages timestamp
);
CREATE TABLE messages(
    chat uuid REFERENCES chats ON DELETE CASCADE,
    sequence integer,
    content varchar NOT NULL,
    sent_on timestamp NOT NULL,
    user_message boolean NOT NULL,
    generation_id varchar,
    finished boolean NOT NULL,
    reasoning varchar,
    input_tokens integer NOT NULL,
    output_tokens integer NOT NULL,
    provider varchar NOT NULL,
    model varchar NOT NULL,
    compacted boolean NOT NULL,
    PRIMARY KEY(chat, sequence)
);
CREATE TABLE tool_calls(
    chat uuid,
    sequence integer,
    call_id varchar,
    tool varchar NOT NULL,
    request varchar NOT NULL,
    response varchar,
    user_consented boolean,
    initiated boolean NOT NULL,
    provider_id varchar,
    PRIMARY KEY(chat, sequence, call_id),
    FOREIGN KEY (chat, sequence) REFERENCES messages ON DELETE CASCADE
);
CREATE TABLE preapproved_code(
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    chat uuid NOT NULL REFERENCES chats ON DELETE CASCADE,
    code varchar NOT NULL
);
CREATE TABLE passkeys(
    id bytea PRIMARY KEY,
    uid uuid NOT NULL REFERENCES users ON DELETE CASCADE,
    passkey varchar NOT NULL,
    created_at timestamp NOT NULL,
    last_used timestamp,
    name varchar NOT NULL
);
CREATE TABLE containers(
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    uid uuid NOT NULL REFERENCES users ON DELETE RESTRICT,
    origin integer NOT NULL,
    state integer NOT NULL,
    created_at timestamp NOT NULL,
    image varchar NOT NULL,
    requested_state integer NOT NULL,
    storage_size integer NOT NULL,
    memory_limit integer NOT NULL
);
CREATE TABLE devices(
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    arch varchar NOT NULL,
    os varchar NOT NULL,
    version varchar NOT NULL,
    pubkey bytea NOT NULL UNIQUE,
    approved boolean NOT NULL,
    hostname varchar NOT NULL UNIQUE,
    first_connection timestamp NOT NULL,
    last_connection timestamp NOT NULL,
    ip varchar NOT NULL,
    agent_hash bytea NOT NULL,
    container_id uuid REFERENCES containers ON DELETE CASCADE
);
CREATE TABLE device_info(
    device uuid REFERENCES devices ON DELETE CASCADE,
    info_type varchar,
    success boolean NOT NULL,
    data varchar NOT NULL,
    updated_timestamp timestamp NOT NULL,
    PRIMARY KEY(device, info_type)
);
CREATE TABLE commands(
    id uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    device uuid NOT NULL REFERENCES devices ON DELETE CASCADE,
    uid uuid REFERENCES users ON DELETE CASCADE,
    code varchar NOT NULL,
    completed boolean NOT NULL,
    status_code integer NOT NULL,
    stdout varchar NOT NULL,
    stderr varchar NOT NULL,
    requested_timestamp timestamp NOT NULL,
    completed_timestamp timestamp,
    origin integer NOT NULL,
    device_info_type varchar,
    chat_id uuid,
    chat_sequence integer,
    chat_tool_call_id varchar,
    initiated boolean NOT NULL,
    language integer NOT NULL,
    transient boolean NOT NULL,
    FOREIGN KEY (chat_id, chat_sequence, chat_tool_call_id) REFERENCES tool_calls ON DELETE CASCADE
);
CREATE TABLE development_environments(
    id uuid PRIMARY KEY REFERENCES containers ON DELETE CASCADE,
    name varchar NOT NULL,
    repo varchar,
    branch varchar,
    nested_containers boolean NOT NULL,
    ssh_key varchar NOT NULL,
    command uuid REFERENCES commands ON DELETE SET NULL
);
CREATE TABLE user_settings(
    uid uuid REFERENCES users ON DELETE CASCADE,
    name varchar NOT NULL,
    value varchar NOT NULL,
    PRIMARY KEY(uid, name)
);
CREATE TABLE agents(
    id uuid PRIMARY KEY REFERENCES containers ON DELETE CASCADE,
    name varchar NOT NULL,
    command uuid REFERENCES commands ON DELETE SET NULL,
    password varchar NOT NULL
);
