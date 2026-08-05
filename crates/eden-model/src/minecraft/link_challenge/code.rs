use core::fmt;
use rand::{RngExt, seq::IndexedRandom};
use secrecy::{ExposeSecret, SecretSlice, SecretString};
use sha2::{Digest, Sha256};

/// A challenge code for Bedrock players who cannot open clickable links.
///
/// Bedrock players memorize a sequence of 4–6 space-separated rainbow colors
/// (e.g. `red orange blue green`) and type them into Discord to prove they
/// control both their Minecraft and Discord accounts.
///
/// The inner value is wrapped in [`SecretString`] so it is zeroized on drop
/// and never accidentally leaked through formatting.
pub struct ChallengeCode(SecretString);

/// Colors of the rainbow (per README.md specification §2).
static RAINBOW_COLORS: &[&str] = &["red", "orange", "yellow", "green", "blue", "violet", "pink"];

const MIN_COLORS: usize = 4;
const MAX_COLORS: usize = 6;

/// Domain prefix prepended to the raw code before hashing, ensuring that
/// SHA-256 digests are scoped to this specific use-case.
const HASH_DOMAIN: &[u8] = b"eden_bedrock_challenge_code_";

impl ChallengeCode {
    /// Parses user input into a validated [`ChallengeCode`].
    ///
    /// Returns [`None`] if any word is not a recognized rainbow color or
    /// the word count falls outside the valid range of 4–6.
    pub fn parse(input: &str) -> Option<Self> {
        let words: Vec<&str> = input.split(' ').collect();

        let valid_count = (MIN_COLORS..=MAX_COLORS).contains(&words.len());
        let valid_colors = words.iter().all(|w| RAINBOW_COLORS.contains(w));

        if !valid_count || !valid_colors {
            return None;
        }

        Some(Self(SecretString::new(input.to_owned().into_boxed_str())))
    }

    /// Generates a random challenge code of 4–6 space-separated rainbow colors.
    ///
    /// This operation is infallible because the color pool is statically
    /// defined and non-empty.
    #[must_use]
    pub fn generate() -> Self {
        let mut rng = rand::rng();
        let count = rng.random_range(MIN_COLORS..=MAX_COLORS);

        let chosen: Vec<&str> = (0..count)
            .map(|_| {
                *RAINBOW_COLORS
                    .choose(&mut rng)
                    .expect("RAINBOW_COLORS must not be empty")
            })
            .collect();

        Self(SecretString::new(chosen.join(" ").into_boxed_str()))
    }

    /// Produces a domain-separated SHA-256 hash of this challenge code.
    ///
    /// The hash is prefixed with `eden_bedrock_challenge_code_` to prevent
    /// cross-protocol collisions.
    #[must_use]
    pub fn hash(&self) -> HashedChallengeCode {
        let mut hasher = Sha256::new();
        hasher.update(HASH_DOMAIN);
        hasher.update(self.expose().as_bytes());

        HashedChallengeCode(hasher.finalize().to_vec().into())
    }

    /// Exposes the underlying challenge code as a string slice.
    ///
    /// Prefer keeping the code wrapped for as long as possible.
    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for ChallengeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ChallengeCode").finish_non_exhaustive()
    }
}

impl fmt::Display for ChallengeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

/// A domain-separated SHA-256 digest of a [`ChallengeCode`].
///
/// Used to store and compare challenge codes without retaining the
/// plaintext in the database.
pub struct HashedChallengeCode(SecretSlice<u8>);

impl HashedChallengeCode {
    /// Returns the raw 32-byte SHA-256 digest.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.expose_secret()
    }

    /// Encodes the digest as a lowercase hexadecimal string.
    #[must_use]
    pub fn encode(&self) -> String {
        hex::encode(self.as_bytes())
    }

    /// Returns `true` if this hash matches another challenge code's hash.
    #[must_use]
    pub fn verify(&self, other: &ChallengeCode) -> bool {
        let candidate = other.hash();
        self.as_bytes() == candidate.as_bytes()
    }
}

impl fmt::Debug for HashedChallengeCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("HashedChallengeCode").finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_generate_valid_challenge_code() {
        let code = ChallengeCode::generate();
        let words: Vec<&str> = code.expose().split(' ').collect();

        assert!(words.len() >= MIN_COLORS);
        assert!(words.len() <= MAX_COLORS);

        for word in &words {
            assert!(RAINBOW_COLORS.contains(word));
        }
    }

    #[test]
    fn should_parse_valid_input() {
        let code = ChallengeCode::parse("red orange blue green");
        assert!(code.is_some());
        assert_eq!(code.unwrap().expose(), "red orange blue green");
    }

    #[test]
    fn should_reject_invalid_colors() {
        assert!(ChallengeCode::parse("red orange blue purple").is_none());
    }

    #[test]
    fn should_reject_too_few_words() {
        assert!(ChallengeCode::parse("red orange blue").is_none());
    }

    #[test]
    fn should_reject_too_many_words() {
        assert!(ChallengeCode::parse("red orange blue green yellow violet pink").is_none());
    }

    #[test]
    fn should_produce_consistent_hash() {
        let a = ChallengeCode::parse("red orange blue green").unwrap();
        let b = ChallengeCode::parse("red orange blue green").unwrap();

        assert_eq!(a.hash().encode(), b.hash().encode());
    }

    #[test]
    fn should_produce_distinct_hashes_for_different_codes() {
        let a = ChallengeCode::parse("red orange blue green").unwrap();
        let b = ChallengeCode::parse("green blue orange red").unwrap();

        assert_ne!(a.hash().encode(), b.hash().encode());
    }

    #[test]
    fn should_verify_matching_code() {
        let code = ChallengeCode::parse("red orange blue green").unwrap();
        let hash = code.hash();

        let same = ChallengeCode::parse("red orange blue green").unwrap();
        assert!(hash.verify(&same));
    }

    #[test]
    fn should_not_verify_different_code() {
        let code = ChallengeCode::parse("red orange blue green").unwrap();
        let hash = code.hash();

        let different = ChallengeCode::parse("green blue orange red").unwrap();
        assert!(!hash.verify(&different));
    }

    #[test]
    fn should_not_leak_secret_in_debug() {
        let code = ChallengeCode::generate();
        let secret = code.expose().to_owned();
        let debug = format!("{code:?}");

        assert!(!debug.contains(&secret));
        assert_eq!(debug, "ChallengeCode(..)");
    }

    #[test]
    fn should_not_leak_secret_in_display() {
        let code = ChallengeCode::generate();
        let display = format!("{code}");

        assert_eq!(display, "<redacted>");
    }

    #[test]
    fn should_encode_hash_as_hex() {
        let code = ChallengeCode::parse("red orange blue green").unwrap();
        let hex = code.hash().encode();

        assert_eq!(hex.len(), 64);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }
}
