use sqlx::Type;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Type)]
#[sqlx(type_name = "mc_edition", rename_all = "lowercase")]
pub enum McEdition {
    Java,
    Bedrock,
}
