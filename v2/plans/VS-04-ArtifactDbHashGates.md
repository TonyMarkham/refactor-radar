# VS-04: Artifact DB And Hash Gates

## Objective

Implement the deterministic artifact DB read model and hash gates in `rr-core`.

This slice turns concrete `rr-data-model` scan records into a delta plan:
unchanged files are skipped, changed files are inspected, and changed or
missing elements are identified.

## Depends On

- VS-01 `rr-data-model`
- VS-03 Rust scanner response shape

## Deliverables

- Add `crates/rr-core`.
- Add artifact DB load/validate helpers.
- Add normalized source file hashing.
- Add file hash gate.
- Add source slice extraction from model ranges.
- Add element content hashing.
- Add deterministic element ID generation.
- Add changed/missing element planning.
- Add tests.

## Artifact DB Shape

The artifact DB shape is owned by `rr-data-model`. `rr-core` owns behavior over
that shape.

Conceptual structure:

```text
ArtifactDb
  Project
    Document
      Element
        Aspect
```

Use JSON initially because the workspace already uses `serde_json`.

Recommended default path:

```text
.refactor-radar/artifacts.json
```

## File Hash Gate

For each scanned document:

```text
current_file_hash = sha256(normalized_file_bytes)
previous_file_hash = artifact_db.projects[project].documents[path].file_hash
```

If hashes match, skip the entire document subtree.

That means no element in that document should be scheduled, including:

- modules
- structs
- enums
- fields
- functions
- methods
- impl-like elements
- tests
- references
- all aspects rooted under the file

Deleted files should be recorded so a later publish slice can expire stale
artifacts. Deleted files do not create summarizer work.

## Element Hash Gate

Only changed or new files proceed to element inspection.

For each element in a changed file:

1. Resolve the source file.
2. Use the current scan range or enclosing range only to locate the source
   slice.
3. Normalize the source slice.
4. Hash only the normalized source slice.

Hash formula:

```text
element_content_hash = sha256(normalized_source_slice_from_scan_bounds)
```

Do not include these values in the content hash:

- SCIP symbol ID
- definition range
- enclosing range
- line number
- column number
- occurrence order
- display name
- parent locator
- signature text as a separate input

Ranges are current-scan locators only.

## Source Normalization

Initial normalization:

```text
normalize CRLF to LF
preserve all non-line-ending whitespace
preserve comments inside the source slice
hash UTF-8 bytes
```

Do not trim or format source slices in v2.

## Element Identity

Element IDs are RefactorRadar-owned and deterministic.

They may include:

- project key
- normalized document path
- element kind
- display name
- deterministic parent locator
- deterministic disambiguator when unavoidable

They must not include:

- raw SCIP symbol IDs
- raw local symbols such as `local 0`
- source range
- line number
- column number

If deterministic identity collides, return a typed duplicate ID error. Do not
fall back to line numbers silently.

## Error Handling

Add `CoreError` and `CoreResult<T>`.

Errors should cover:

- artifact DB read failure
- artifact DB parse failure
- unsupported schema version
- source file missing
- unsupported position encoding
- source range out of bounds
- invalid source slice
- duplicate deterministic element ID
- ambiguous parent locator

Use the repo typed error policy.

## Tests

Add tests for:

- missing artifact DB loads as empty
- malformed artifact DB returns typed error
- unchanged file skips every child element
- changed file inspects children
- blank line above an element does not change its content hash
- function body change changes only that element hash
- field type change changes that field hash
- sibling method change does not change unchanged sibling method hash
- range values are not content hash inputs
- duplicate deterministic IDs return typed errors

## Verification

Run:

```bash
cargo test -p rr-core
cargo check -p rr-core
```

## Non-Goals

- Do not invoke language scanner binaries.
- Do not expose MCP tools.
- Do not spawn summarizer subagents.
- Do not publish summary artifacts.
