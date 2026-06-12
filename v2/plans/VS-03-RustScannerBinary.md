# VS-03: Rust Scanner Binary

## Objective

Add `crates/rr-scip-rust` as the Rust language scanner binary.

The binary accepts a Rust crate or workspace target, runs `rust-analyzer scip`,
parses the generated SCIP artifact through `rr-scip`, and emits concrete
`rr-data-model` scan records.

## Depends On

- VS-01 `rr-data-model`
- VS-02 `rr-scip`

## Deliverables

- Add `crates/rr-scip-rust`.
- Add a `rr-scip-rust` binary.
- Define scanner request and response file handling.
- Run `rust-analyzer scip`.
- Save the generated `.scip` artifact.
- Convert Rust/rust-analyzer SCIP data into `rr-data-model` records.
- Preserve every serialized SCIP symbol as an artifact-eligible element unless
  malformed.
- Add tests and fixtures.

## Process Contract

The binary should support:

```bash
rr-scip-rust scan --request request.json --out scan-response.json
```

Request fields should include:

- target path
- target kind
- run directory
- rust-analyzer path
- rust-analyzer config path
- exclude vendored libraries flag
- thread count
- output SCIP artifact path

Response fields should include:

- scanner ID
- scanner version
- target path
- generated SCIP artifact path
- documents
- elements
- occurrences or references needed by summaries
- diagnostics

Use `rr-data-model` shapes for request and response where possible.

## Rust Scanner Responsibilities

The Rust scanner owns rust-analyzer-specific behavior:

- command construction for `rust-analyzer scip`
- rust-analyzer stderr capture
- rust-analyzer failure diagnostics
- parsing rust-analyzer SCIP output through `rr-scip`
- conversion from `scip::types::Index` into `rr-data-model`
- handling rust-analyzer quirks such as synthetic `impl#[Type]` symbols

The Rust scanner should not:

- compare artifact DB entries
- compute file hash gates
- compute element content hashes
- schedule work items
- expose MCP tools

## Artifact Eligibility

Every serialized SCIP symbol should become an `rr-data-model` element unless it
is malformed.

Do not filter to a hand-picked list of Rust kinds.

Rust-specific examples:

- `source_span/impl#[SourceSpan]` should become an element.
- It may retain `kind: TypeAlias` if that is what rust-analyzer emitted.
- The summary layer can later describe it as a Rust impl block.

The scanner preserves evidence. It does not need to explain language nuance in
prose.

## Source Bounds

For each element, preserve source bounds sufficient for `rr-core` to later
slice the source file:

- document path
- symbol
- display name
- symbol kind
- definition range if present
- enclosing range if present
- position encoding

Ranges remain current-scan coordinates. They are not durable IDs and are not
content hash inputs.

## Error Handling

Add `RustScannerError` and `RustScannerResult<T>`.

Errors should cover:

- malformed request
- target path missing
- rust-analyzer missing
- rust-analyzer launch failure
- rust-analyzer nonzero exit
- SCIP artifact missing
- SCIP parse failure
- data-model conversion failure
- response write failure

Use the repo typed error policy.

## Tests

Add tests for:

- request JSON parsing
- response JSON serialization
- rust-analyzer command construction
- conversion preserves all document paths
- conversion preserves all serialized symbols as elements
- conversion preserves `impl#[Type]` symbols
- conversion preserves enclosing ranges
- malformed SCIP symbol creates diagnostic or typed error

Use fixtures for conversion tests. Do not require rust-analyzer for every unit
test.

## Verification

Run:

```bash
cargo test -p rr-scip-rust
cargo check -p rr-scip-rust
```

Manual verification:

```bash
cargo run -p rr-scip-rust -- scan \
  --request .refactor-radar/dev/rust-scan-request.json \
  --out .refactor-radar/dev/rust-scan-response.json
```

## Non-Goals

- Do not implement C# scanning.
- Do not implement artifact DB comparison.
- Do not implement MCP tools.
- Do not generate summary artifacts.
