## [`eden-config-derive`](.)
Procedural macros for the `eden-config` crate that provides the [`Validate`] derive for automatically implement configuration validation.

### Usage
```rust
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Validate)]
pub struct Bot {
    // Fields are validated by default
    pub name: String,

    // Skips validation for this field
    #[validate(skip)]
    pub optional_field: u32,
    
    // Custom validation with `with = ...` metadata
    #[validate(with = "validate_port")]
    pub port: u16,
}

fn validate_port(
    port: &u16,
    ctx: &crate::validation::ValidationContext<'_>
) -> Result<(), eden_file_diagnostics::RenderedDiagnostic> {
    if *port <= 0 {
        // Return diagnostic error
    }
    Ok(())
}
```
