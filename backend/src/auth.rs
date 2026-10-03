//! Password hashing.
//!
//! New passwords are stored as argon2id PHC strings. Accounts imported from
//! musicbanana-php only have an unsalted MD5 hex digest; those are stored as
//! argon2id(md5_hex(password)) with `account.password_legacy_md5 = true`, so no
//! weak hash is ever at rest. After the next successful login the caller should
//! replace the hash with `hash_password(password)` and clear the flag.

use argon2::{
    Argon2,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use md5::{Digest, Md5};

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    Ok(Argon2::default()
        .hash_password(password.as_bytes())
        .map_err(|e| anyhow::anyhow!("hashing password: {e}"))?
        .to_string())
}

/// Wraps a legacy MD5 hex digest (as stored by musicbanana-php) in argon2id.
pub fn wrap_legacy_md5(md5_hex: &str) -> anyhow::Result<String> {
    hash_password(&md5_hex.trim().to_ascii_lowercase())
}

pub fn verify_password(stored: &str, legacy_md5: bool, password: &str) -> bool {
    let candidate = if legacy_md5 {
        md5_hex(password)
    } else {
        password.to_owned()
    };
    PasswordHash::new(stored).is_ok_and(|hash| {
        Argon2::default()
            .verify_password(candidate.as_bytes(), &hash)
            .is_ok()
    })
}

fn md5_hex(input: &str) -> String {
    Md5::digest(input.as_bytes())
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5_hex_matches_php_md5() {
        // php -r 'echo md5("banana");'
        assert_eq!(md5_hex("banana"), "72b302bf297a228a75730123efef7c41");
    }

    #[test]
    fn wrapped_legacy_hash_verifies_with_the_plain_password() {
        let stored = wrap_legacy_md5("72B302BF297A228A75730123EFEF7C41").unwrap();
        assert!(verify_password(&stored, true, "banana"));
        assert!(!verify_password(&stored, true, "apple"));
        // A leaked MD5 from the old database must not work as a password.
        assert!(!verify_password(
            &stored,
            true,
            "72b302bf297a228a75730123efef7c41"
        ));
    }

    #[test]
    fn modern_hash_round_trips() {
        let stored = hash_password("banana").unwrap();
        assert!(stored.starts_with("$argon2id$"));
        assert!(verify_password(&stored, false, "banana"));
        assert!(!verify_password(&stored, true, "banana"));
    }
}
