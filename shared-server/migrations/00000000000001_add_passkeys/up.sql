CREATE TABLE passkeys(
  id bytea PRIMARY KEY,
  uid uuid NOT NULL REFERENCES users ON DELETE CASCADE,
  passkey varchar NOT NULL
);
