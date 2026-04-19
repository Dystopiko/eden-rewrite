use doku::Document;
use eden_config_derive::Validate;
use serde::Deserialize;
use std::path::PathBuf;

#[derive(Clone, Debug, Deserialize, Document, Eq, PartialEq, Validate)]
pub struct Tls {
    #[validate(with = "super::validators::check_for_tls_cert_file")]
    pub cert_file: PathBuf,

    #[validate(with = "super::validators::check_for_tls_priv_key_file")]
    pub priv_key_file: PathBuf,
}
