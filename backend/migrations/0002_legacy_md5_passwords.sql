-- Accounts imported from musicbanana-php only have an unsalted MD5 of the password.
-- They are stored as password_hash = argon2id(md5_hex(password)) with this flag set;
-- after the next successful login the hash becomes argon2id(password) and the flag
-- is cleared (see src/auth.rs).
ALTER TABLE account ADD COLUMN password_legacy_md5 boolean NOT NULL DEFAULT false;
