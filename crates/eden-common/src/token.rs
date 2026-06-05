use eden_model::tables::tokens::TokenType;
use rand::seq::IndexedRandom;
use rand_chacha::ChaCha20Rng;
use secrecy::{ExposeSecret, SecretSlice, SecretString};
use sha2::{Digest, Sha256};
use std::fmt;

#[must_use]
pub struct RawToken(SecretString);

impl RawToken {
    #[must_use]
    pub fn parse(inner: String) -> Option<Self> {
        static TOKEN_PREFIXES: &[&str] = &[USER_TOKEN_PREFIX, MC_SERVER_TOKEN_PREFIX];
        TOKEN_PREFIXES
            .iter()
            .any(|prefix| inner.starts_with(prefix))
            .then(|| Self(inner.into()))
    }

    pub fn generate(kind: TokenType) -> Self {
        let mut rng: ChaCha20Rng = rand::make_rng();
        let chars: String = CHARSET
            .choose_iter(&mut rng)
            .expect("CHARSET is not empty")
            .take(GENERATED_CHARS_LENGTH)
            .map(|&b| b as char)
            .collect();

        let prefix = match kind {
            TokenType::McServer => MC_SERVER_TOKEN_PREFIX,
            TokenType::User => USER_TOKEN_PREFIX,
        };

        Self(format!("{prefix}{chars}").into())
    }

    pub fn hash(&self) -> HashedToken {
        let bytes = Sha256::digest(self.expose().as_bytes()).as_slice().to_vec();
        HashedToken(bytes.into())
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

const USER_TOKEN_PREFIX: &str = "eden_ut_";
const MC_SERVER_TOKEN_PREFIX: &str = "eden_mcst_";

const GENERATED_CHARS_LENGTH: usize = 70;
const CHARSET: &[u8] = b"\
    ABCDEFGHIJKLMNOPQRSTUVWXYZ\
    abcdefghijklmnopqrstuvwxyz\
    0123456789\
    _";

impl fmt::Debug for RawToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RawToken([redacted])")
    }
}

#[must_use]
pub struct HashedToken(SecretSlice<u8>);

impl HashedToken {
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.expose_secret()
    }

    #[must_use]
    pub fn encode(&self) -> String {
        hex::encode(self.as_bytes())
    }
}

impl fmt::Debug for HashedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HashedToken([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use super::{CHARSET, RawToken};
    use claims::assert_ok;
    use eden_model::tables::tokens::TokenType;

    #[test]
    fn token_is_valid_utf8() {
        let token = RawToken::generate(TokenType::McServer);
        assert_ok!(String::from_utf8(token.expose().as_bytes().to_vec()));
    }

    #[test]
    fn tokens_are_unique() {
        let token1 = RawToken::generate(TokenType::User);
        let token2 = RawToken::generate(TokenType::User);
        assert_ne!(token1.expose(), token2.expose());
    }

    #[test]
    fn token_only_contains_valid_charset_characters() {
        let token = RawToken::generate(TokenType::User);
        for ch in token.expose().bytes() {
            assert!(CHARSET.contains(&ch), "character '{ch}' is not in CHARSET");
        }
    }

    #[test]
    fn hashing_same_token_twice_yields_same_hash() {
        let token = RawToken::generate(TokenType::User);
        let hash1 = token.hash();
        let hash2 = token.hash();
        assert_eq!(hash1.as_bytes(), hash2.as_bytes());
    }

    #[test]
    fn different_tokens_yield_different_hashes() {
        let token1 = RawToken::generate(TokenType::User);
        let token2 = RawToken::generate(TokenType::McServer);
        assert_ne!(token1.hash().as_bytes(), token2.hash().as_bytes());
    }

    #[test]
    fn debug_output_is_redacted() {
        let token = RawToken::generate(TokenType::McServer);
        assert_eq!(format!("{token:?}"), "RawToken([redacted])");

        let hash = token.hash();
        assert_eq!(format!("{hash:?}"), "HashedToken([redacted])");
    }
}
