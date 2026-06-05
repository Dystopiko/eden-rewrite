use eden_human_input::generator::random_words;
use error_stack::Report;
use rand_chacha::ChaCha20Rng;
use secrecy::{ExposeSecret, SecretSlice, SecretString};
use sha2::{Digest, Sha256};
use std::fmt;
use thiserror::Error;

/// Returned when the word generator is exhausted before the challenge code
/// satisfies both the minimum word count and minimum character length.
#[derive(Debug, Error)]
#[error("failed to roll a generated word")]
pub struct RollWordError;

struct ChallengeConfig {
    separator: char,
    min_words: usize,
    min_chars: usize,
}

impl ChallengeConfig {
    /// Configuration for Java edition accounts.
    const JAVA: Self = Self {
        separator: '-',
        min_words: 3,
        min_chars: 25,
    };

    /// Configuration for Bedrock edition accounts.
    ///
    const BEDROCK: Self = Self {
        // Some of our Bedrock users have to switch to Discord app to order to send the complex
        // challenge code which is easier for Java users, we can make it only spaces.
        separator: ' ',
        min_words: 2,
        min_chars: 10,
    };
}

/// A freshly generated, unhashed challenge code.
#[must_use]
pub struct RawChallengeCode(SecretString);

impl RawChallengeCode {
    /// Generates a challenge code suitable for Java edition accounts.
    pub fn generate_for_java() -> Result<Self, Report<RollWordError>> {
        Self::generate(&ChallengeConfig::JAVA)
    }

    /// Generates a challenge code suitable for Bedrock edition accounts.
    pub fn generate_for_bedrock() -> Result<Self, Report<RollWordError>> {
        Self::generate(&ChallengeConfig::BEDROCK)
    }

    /// Attempts to parse a possible challenge code.
    ///
    /// Accepts codes in either Java format (tried first) or Bedrock format.
    pub fn parse(content: &str) -> Option<Self> {
        [ChallengeConfig::JAVA, ChallengeConfig::BEDROCK]
            .into_iter()
            .any(|config| Self::matches_config(content, &config))
            .then(|| RawChallengeCode(content.to_string().into()))
    }

    /// Returns whether `content` satisfies the word count and character length
    /// requirements of `config` when split on `config.separator`.
    fn matches_config(content: &str, config: &ChallengeConfig) -> bool {
        let word_count = content.split(config.separator).count();
        word_count >= config.min_words && content.len() >= config.min_chars
    }

    /// Returns [`RollWordError`] if the iterator yields no further words before
    /// the thresholds are met.
    fn generate(config: &ChallengeConfig) -> Result<Self, Report<RollWordError>> {
        let mut code = String::with_capacity(config.min_chars);
        let mut rng: ChaCha20Rng = rand::make_rng();

        for (word_index, word) in random_words(&mut rng).enumerate() {
            if !code.is_empty() {
                code.push(config.separator);
            }

            code.push_str(word);

            if word_index + 1 >= config.min_words && code.len() >= config.min_chars {
                return Ok(RawChallengeCode(code.into()));
            }
        }

        Err(Report::new(RollWordError))
    }

    /// Generates a hashed challenge code.
    pub fn hash(&self) -> HashedChallengeCode {
        let mut hasher = Sha256::new();
        hasher.update(b"eden_challenge_code_");
        hasher.update(self.expose().as_bytes());

        HashedChallengeCode(hasher.finalize().to_vec().into())
    }

    /// Returns the plaintext code value.
    ///
    /// Prefer keeping the code wrapped for as long as possible.
    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for RawChallengeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RawChallengeCode([redacted])")
    }
}

/// A hashed challenge code.
#[must_use]
pub struct HashedChallengeCode(SecretSlice<u8>);

impl HashedChallengeCode {
    /// Returns the raw hash bytes.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.expose_secret()
    }

    /// Encodes the hash bytes as a lowercase hexadecimal string.
    #[must_use]
    pub fn encode(&self) -> String {
        hex::encode(self.as_bytes())
    }
}

impl fmt::Debug for HashedChallengeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("HashedChallengeCode([redacted])")
    }
}

#[cfg(test)]
mod tests {
    use crate::challenge_code::RawChallengeCode;

    #[test]
    fn should_be_generate_unique_codes() {
        let code1 = RawChallengeCode::generate_for_java().unwrap();
        let code2 = RawChallengeCode::generate_for_java().unwrap();
        assert_ne!(code1.expose(), code2.expose());

        let code1 = RawChallengeCode::generate_for_bedrock().unwrap();
        let code2 = RawChallengeCode::generate_for_bedrock().unwrap();
        assert_ne!(code1.expose(), code2.expose());
    }

    #[test]
    fn should_generate_same_hash_for_the_same_code() {
        let code = RawChallengeCode::generate_for_java().unwrap();
        let hash1 = code.hash().encode();
        let hash2 = code.hash().encode();
        assert_eq!(hash1, hash2);

        let code = RawChallengeCode::generate_for_bedrock().unwrap();
        let hash1 = code.hash().encode();
        let hash2 = code.hash().encode();
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn should_redact_debug_output() {
        let code = RawChallengeCode::generate_for_java().unwrap();
        assert_eq!(format!("{code:?}"), "RawChallengeCode([redacted])");

        let code = RawChallengeCode::generate_for_bedrock().unwrap();
        assert_eq!(format!("{code:?}"), "RawChallengeCode([redacted])");
    }
}
