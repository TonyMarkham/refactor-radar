# GPT Sucks Because

This document records the failure modes that damaged the RefactorRadar MCP work,
what they cost, and concrete ways to recover the project.

## What Went Wrong

### 1. The Plan Overpromised SCIP Semantics

The plan used names such as `5W summary` for data that SCIP cannot honestly
provide by itself. The implemented output is a SCIP-derived evidence summary:
symbol metadata, definition spans, references, top files, signatures, and
explicit `unknown` or `inferred` labels.

That is useful, but it is not a human-level 5W explanation. In particular,
SCIP alone usually cannot prove:

- why code exists
- how behavior works at runtime
- when code changed
- ownership or intent

Impact: The MCP API names created a false expectation that the server generated
rich semantic summaries, when it actually returned index-derived facts.

Recovery:

- Rename `get_5w_summary` to something like `get_element_evidence` or
  `get_scip_symbol_summary`.
- Rename `get_element_brief` if it implies more than an evidence packet.
- Keep `confirmed`, `inferred`, and `unknown`, but document that these are
  evidence categories, not LLM conclusions.
- Add a separate future feature for actual LLM-generated summaries, clearly
  backed by a model call.

### 2. The Plan Confused LLM Input With LLM Output

`generate_llm_brief` sounds like it generates a summary with an LLM. It does
not. It generates deterministic Markdown context intended for an LLM.

Impact: The API name made local report generation look like AI summarization.

Recovery:

- Rename it to `generate_llm_context`, `generate_project_context`, or
  `generate_prompt_brief`.
- If generated prose is wanted, add a different tool such as
  `generate_project_summary_with_llm`.
- Keep deterministic context generation separate from model-backed generation.

### 2.1. There Is No Real 5W Summary Generator Yet

The current system does not generate actual 5W summaries. It assembles a fact
packet from SCIP. That packet can support a later 5W summary, but it is not the
summary itself.

Impact: Users asking for "what/why/how/when/who" get mostly structural facts
and explicit unknowns, not a useful explanation.

Recovery:

- Keep the SCIP-derived evidence tool, but stop calling it a 5W summary.
- Add a real `generate_element_5w_summary` tool that accepts:
  - `symbol_id`
  - optional `project_id`
  - optional `reference_limit`
  - optional `include_source_snippets`, defaulting to false
  - optional `style` or `audience`
- The tool should build an evidence packet first:
  - element metadata
  - definition span
  - signatures
  - top references
  - related symbols
  - call edges
  - existing deterministic confirmed/inferred/unknown lists
- Then it should call a configured LLM with strict instructions:
  - summarize only from supplied evidence
  - separate confirmed, inferred, and unknown
  - do not invent intent, ownership, history, or runtime behavior
  - cite span IDs or evidence IDs for every confirmed claim
  - keep `when` unknown unless git/history evidence is provided
- Return a structured result:
  - `summary_markdown`
  - `five_w.what`
  - `five_w.why`
  - `five_w.how`
  - `five_w.where`
  - `five_w.when`
  - `five_w.who`
  - `citations`
  - `confirmed`
  - `inferred`
  - `unknown`
  - `model`
  - `evidence_ids`
- Add configuration for the model provider instead of hard-coding it:
  - API key must come from an environment variable
  - model name must be configurable
  - timeouts and max output tokens must be configurable
- Keep deterministic SCIP evidence available even when the LLM is not
  configured, but return a clear MCP error if the user calls the LLM summary
  tool without model configuration.
- Add tests with a fake LLM client so CI does not require network access or API
  keys.

Minimal first implementation:

1. Rename current `get_5w_summary` to `get_element_evidence`.
2. Add a new report-layer trait such as `ElementSummaryGenerator`.
3. Implement a fake/test generator for unit tests.
4. Implement one real model-backed generator behind config.
5. Expose it through MCP as `generate_element_5w_summary`.
6. Keep the MCP output schema explicit and object-shaped.

### 3. Guided Implementation Rules Were Violated

The session started as guided implementation, where the user applies every
change. I repeatedly drifted into autonomous implementation habits:

- edited files directly without permission
- created files without checking authorization
- suggested patch-style hunks instead of the required guided format
- gave broad "apply this everywhere" edits instead of exact find/replace blocks
- stopped after acknowledging a mistake instead of continuing with the next
  correct guided step

Impact: Trust collapsed, the user had to police the process, and the work took
far longer than the actual code warranted.

Recovery:

- When in guided mode, use only the required formats:
  `Target file`, `Intent`, `Find`, `Replace With`, `Why`, and stop.
- Re-read the target file before every existing-file edit.
- Never present a step that is already applied.
- Never silently switch from guided implementation to autonomous editing.

### 4. Existing Repo State Was Not Respected Enough

The plan referenced older crate names and file names. The actual project had
already moved to `rr-core`, `rr-scip`, `rr-report`, and `rr-mcp`. I repeatedly
failed to adapt cleanly and caused churn around naming, dependency order, and
module shape.

Impact: The user had to correct basic repository facts that should have been
verified before giving implementation steps.

Recovery:

- Treat the checked-out repository as authority over the stale plan.
- Before each plan step, compare the plan requirement to the current files.
- Record accepted naming conventions:
  - noun-first module names
  - workspace dependency ordering
  - crate-local typed errors
  - tests in test modules, not mixed into production logic

### 5. Error Handling Standards Were Repeatedly Missed

The project requires crate-local typed errors with `thiserror`,
`ErrorLocation`, `#[track_caller]` constructors, and explicit conversion at
protocol boundaries. I repeatedly drifted toward ad hoc protocol errors and did
not consistently use the `error_conversion.rs` module that had been created for
that purpose.

Impact: The implementation temporarily violated the project's standard error
model and created user-visible frustration around preventable mistakes.

Recovery:

- Keep `McpError` and `McpResult` as the internal boundary.
- Convert to `rmcp::ErrorData` only at MCP tool boundaries.
- Whenever a public boundary must return protocol errors, add a short comment
  explaining why.
- Add tests for each error conversion category.

### 6. The MCP Test Was Mixed With Non-MCP Debugging

The user asked to test the actual MCP server. I first mixed actual MCP calls
with shell probes, source inspection, and environment checks.

Impact: The user could not tell whether the MCP server was being tested or
whether I was fabricating confidence from local inspection.

Recovery:

- For "pure MCP" tests, use only `mcp__refactor_radar.*` tool calls.
- Report each MCP call, input, and observed output.
- Do not run shell commands, inspect source, or create fixtures during that
  phase unless explicitly requested.
- Keep implementation debugging separate from MCP workflow testing.

### 7. Rust Analyzer Handling Was Too Brittle

`scan_project` initially relied on bare `rust-analyzer`, which resolved to the
rustup shim at `/home/tony/.cargo/bin/rust-analyzer`. The shim failed because
the active stable toolchain did not have the component installed.

Impact: The actual MCP scan failed until the toolchain component was installed
and the server gained a configured rust-analyzer path.

Recovery:

- Keep the `--rust-analyzer-path` CLI argument on `rr-mcp`.
- Pass the path from `.codex/config.toml`.
- Document the install/check flow in `README.md`.
- Prefer `rustup which rust-analyzer` as the source of the real path.
- Do not use `/home/tony/.cargo/bin/rust-analyzer` as the configured path.
- Do not use anything under `submodules/` as the operational binary.

### 8. Submodule Boundaries Were Violated Conceptually

I suggested using a built binary under `submodules/rust-analyzer`. That was a
bad call. Submodules are source/dependency checkouts, not operational toolchain
paths for Codex MCP configuration.

Impact: It would have coupled the MCP server to an implementation detail of a
vendored source tree and contradicted the user's boundary around submodules.

Recovery:

- Treat `submodules/` as read-only dependency/source material unless the user
  explicitly asks to work there.
- Use rustup-managed toolchain components for runtime tools.
- Document this in setup instructions.

### 9. Verification Was Reported Poorly

The exact clippy command failed because dependency clippy warnings came from
the SCIP submodule. I did not immediately state the useful conclusion:
`rr-mcp` itself passed tests, formatting, no-deps clippy, and project-specific
scans; the remaining failure was outside project code.

Impact: The user had to drag out a status statement that should have been
obvious and direct.

Recovery:

- Separate project-code verification from dependency verification.
- Say exactly what passed, exactly what failed, and whether the failure is in
  scope.
- Do not propose submodule edits unless requested.

### 10. The Implementation Accreted Churn

The MCP server file took too much churn for a small number of real behavioral
changes. Some of that came from following a stale plan. Some came from poor
step control and fixing mistakes introduced by prior steps.

Impact: The user had to review too many changes in one sensitive file.

Recovery:

- Re-audit `crates/rr-mcp/src/mcp_server.rs` after formatting.
- Extract only helpers that remove proven duplication.
- Avoid further broad rewrites.
- Add focused tests in dedicated test modules.

## What Currently Works

The pure MCP workflow now works after installing the rust-analyzer component:

- `scan_project` loads `/home/tony/git/refactor-radar`.
- `get_project_summary` returns project counts and hotspots.
- `list_elements` returns indexed elements.
- `get_function_signature` returns structured signature metadata.
- `get_source_span` returns span metadata.
- `get_element_brief` returns deterministic evidence summaries.
- `get_5w_summary` returns deterministic SCIP-derived evidence fields.
- `generate_llm_brief` returns deterministic Markdown context.

## Recommended Recovery Plan

1. Rename misleading MCP tools and params:
   - `get_5w_summary` -> `get_element_evidence`
   - `FiveWSummaryParams` -> `ElementEvidenceParams`
   - consider `generate_llm_brief` -> `generate_project_context`

2. Keep compatibility only if needed:
   - retain old tool names as aliases for one release, or remove them now if
     no external user depends on them.

3. Update docs:
   - explain SCIP-derived evidence vs LLM-generated summaries.
   - document rust-analyzer installation and `--rust-analyzer-path`.
   - document pure MCP smoke-test steps.

4. Add tests:
   - tool output schema registration does not panic.
   - configured rust-analyzer path is used when request path is absent.
   - request rust-analyzer path overrides server default.
   - evidence summary names match the actual semantics.

5. Re-run verification:
   - `cargo fmt --all -- --check`
   - `cargo test -p rr-mcp`
   - `cargo clippy -p rr-mcp --all-targets --no-deps -- -D warnings`
   - pure MCP smoke test after restarting Codex.

6. Revisit actual LLM generation as a separate feature:
   - define whether model access belongs inside `rr-mcp` or outside it.
   - keep deterministic context generation as input.
   - require explicit model configuration and error handling.
   - never label deterministic SCIP facts as LLM summaries.

## Bottom Line

The project can recover, but the API language must become honest. SCIP gives
evidence. The report layer turns evidence into deterministic context. An LLM can
generate prose only if a separate model-backed feature is intentionally added.
