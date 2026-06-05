# RefactorRadar Agent Guide

## Rust Error Handling

Use crate-local typed errors with `thiserror`, `error_location::ErrorLocation`,
and `#[track_caller]` constructor functions.

Do not use `anyhow` for application or library errors unless explicitly
requested for throwaway tooling or spike code.

Each crate should define its own error enum and result alias:

```rust
pub enum CoreError {
    // ...
}

pub type CoreResult<T> = std::result::Result<T, CoreError>;
```

Error variants should carry `location: ErrorLocation`, and public constructor
functions should attach `ErrorLocation::from(Location::caller())`.

Prefer precise domain variants over broad catch-all errors. Provide stable plain
messages separately, such as through `message()`, when callers need mapping text
without source-location details.
