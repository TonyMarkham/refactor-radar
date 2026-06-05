# ADR/PRD 0001: RefactorRadar Project Direction

Status: Draft
Date: 2026-06-04
Project Name: RefactorRadar
License Intent: MIT
Primary Implementation Target: Rust
Initial Language Support: Rust
Planned Language Support: C#
Primary Integration Target: CLI first, MCP later

## 1. Summary

RefactorRadar is a local-first, MIT-licensed semantic code scanner for generating compact codebase summaries, review context, and eventually evidence-backed refactor opportunity reports.

The initial goal is deliberately modest:

> Parse a codebase, build a useful semantic model, and generate summaries that help humans and LLM agents understand the code before planning changes.

RefactorRadar should not start as an automatic refactoring tool, a linter, a code-quality gate, or a commercial code review platform. It should start as a practical developer tool that answers:

* What is in this codebase?
* What are the important modules, types, functions, methods, traits, interfaces, and tests?
* What calls what?
* What public API surface exists?
* What error/result shapes exist?
* What are the likely review hotspots?
* What context should a human or LLM see before reviewing or planning changes?

Over time, RefactorRadar can grow additional analysis layers for repeated patterns, maintainability drift, refactor candidates, before/after comparison, git-history weighting, and agentic orchestration.

The long-term vision is:

> A local-first semantic review substrate that helps humans and coding agents reason about large codebases using compact, evidence-preserving summaries.

## 2. Context

Large codebases are hard to reason about because the important design signals are spread across many files, modules, crates, projects, and architectural boundaries.

This is true for traditional codebases and for LLM-assisted development.

In traditional codebases, maintainability drift often appears gradually:

* related concepts get named differently
* similar workflows grow independently
* error mapping logic spreads across layers
* modules accumulate unclear responsibilities
* tests develop repeated setup patterns
* public API surfaces expand accidentally
* duplication becomes normalized because no one sees the whole shape

In LLM-assisted development, the same problem can appear faster because LLMs are very effective at building focused vertical slices. That is useful, but repeated slices can independently produce similar local solutions.

This does not mean the code is bad. In many cases, duplication is part of discovery. The repeated shapes are useful signals. They may indicate that an abstraction is emerging naturally.

RefactorRadar should help developers see those signals.

The development rhythm this project should support is:

```text
build focused slice
build focused slice
build focused slice
scan semantic structure
review summaries
identify possible convergence targets
plan focused cleanup
verify improvement
continue building
```

The first useful version does not need to detect every refactor opportunity. It only needs to produce trustworthy semantic summaries and LLM-ready review context.

## 3. Decision

We will build RefactorRadar as a tree-sitter-powered semantic code scanner.

The first implementation will focus on:

1. scanning Rust source code
2. extracting a compact semantic model
3. writing structured JSON artifacts
4. rendering Markdown summaries
5. generating LLM-ready review briefs
6. preserving source spans and stable identifiers
7. supporting future MCP integration

Refactor opportunity detection will be treated as an incremental feature built on top of the semantic model, not as the first milestone.

The core design decision is:

> Deterministic tools produce facts. LLM agents produce interpretation. Humans make design decisions.

## 4. Product Positioning

RefactorRadar is a semantic scanner for codebase review context.

It is not:

* a linter
* a formatter
* a code review gate
* an automatic refactoring engine
* a replacement for compiler-grade semantic analysis
* a replacement for human design judgment
* an enterprise code-quality dashboard
* a tool only for LLM-generated code

It is:

* a local-first semantic code scanner
* a codebase summary generator
* an LLM-ready context generator
* a future MCP-accessible code intelligence tool
* a foundation for evidence-backed refactor discovery

The public positioning should avoid negative framing such as “AI slop” as the main product language.

Preferred terms:

* semantic model
* codebase summary
* maintainability drift
* review context
* structural signals
* refactor candidates
* convergence targets
* evidence-backed cleanup
* emerging abstraction
* repeated structural pattern

## 5. Philosophy

RefactorRadar should find signals, not issue judgments.

Duplication is allowed during discovery.
Abstraction is earned after repetition.
Cleanup should be narrow, evidence-based, and reversible.

The tool should preserve the distinction between confirmed facts, inference, and unknowns.

```text
Confirmed:
- Facts observed directly from source structure.

Inferred:
- Possible interpretations based on naming, structure, and repetition.

Unknown:
- Domain or behavioral details requiring human judgment.
```

The scanner should avoid claiming that code “must” be refactored. It should surface evidence that a human or LLM can use to decide whether a refactor is appropriate.

Bad framing:

```text
This code is duplicated and should be abstracted.
```

Better framing:

```text
These three modules contain similar call sequences and error-mapping shapes.

Confirmed:
- The same status mapping appears in three locations.
- The same domain terms appear in all three workflows.

Inferred:
- There may be an emerging shared boundary.

Unknown:
- Whether the workflows share the same domain semantics.

Suggested next step:
- Compare the spans directly before creating a shared abstraction.
```

## 6. License and Project Intent

RefactorRadar is intended to be MIT-licensed open source.

The project is not intended to become a business or SaaS product. It is primarily a personal developer tool. If others find it useful, they should be able to use it, fork it, modify it, embed it, or contribute with minimal friction.

This has important product implications.

RefactorRadar does not need:

* billing
* cloud accounts
* hosted dashboards
* user management
* enterprise compliance features
* team permission models
* PR decoration
* quality gates
* SaaS deployment
* multi-tenant infrastructure

RefactorRadar should prioritize:

* local execution
* simple CLI workflows
* inspectable JSON artifacts
* useful Markdown reports
* reproducible scans
* agent-friendly query interfaces
* low-friction setup

## 7. Primary Use Cases

### 7.1 Human Codebase Review

A developer wants to understand a crate, project, workspace, or solution before making changes.

RefactorRadar should generate a summary containing:

* project structure
* important symbols
* public API surface
* major modules/namespaces
* significant functions/methods
* tests
* call summaries
* error/result shapes
* possible review hotspots

### 7.2 LLM-Assisted Planning

A developer wants to ask Codex, OpenCode, ChatGPT, or another LLM to review a codebase or produce an implementation plan.

Instead of pasting raw files, the developer can generate an LLM-ready brief:

```bash
refactor-radar brief ./crates/orders --budget 80000 --out orders.llm.md
```

The LLM receives structured context rather than raw source noise.

### 7.3 MCP-Based Agent Queries

A coding agent should be able to query the semantic model on demand.

Example:

```text
Agent:
List the important public types in the orders crate.

RefactorRadar MCP:
Returns compact JSON with type IDs and source spans.

Agent:
Get focused evidence for the repeated error-mapping candidate.

RefactorRadar MCP:
Returns related spans, symbol summaries, and candidate notes.
```

### 7.4 Refactor Discovery

Future versions should identify repeated structures and maintainability drift.

Examples:

* repeated call sequences
* similar enum variant sets
* repeated error mapping
* repeated validation flows
* repeated test setup
* naming drift
* cross-crate concept duplication
* public API surface growth
* dependency boundary pressure

### 7.5 Before/After Verification

After a cleanup, the developer should be able to compare scans:

```bash
refactor-radar compare .radar/before .radar/after --out comparison.md
```

The comparison should answer:

* Did the repeated pattern decrease?
* Did complexity move or disappear?
* Did public API surface expand?
* Did dependency direction improve or worsen?
* Were new risks introduced?

## 8. Non-Goals

The MVP will not:

* automatically refactor code
* generate patches
* enforce coding standards
* block commits
* replace static analyzers such as SonarQube
* replace behavioral code-health tools such as CodeScene
* replace code search tools
* guarantee semantic equivalence
* require an LLM provider
* require Codex CLI
* require MCP
* support every language
* perform compiler-grade semantic analysis
* infer full type information
* solve large-repo summarization in a single context window

Automatic patch generation may be explored later, but only after semantic modeling, summaries, evidence IDs, and verification workflows are stable.

## 9. Deterministic Tools vs. LLM Agents

RefactorRadar should clearly separate deterministic source analysis from LLM-based interpretation.

The deterministic layer is responsible for producing repeatable facts.
The LLM layer is responsible for summarizing, interpreting, comparing, ranking, and planning from those facts.

This separation is important because the semantic model must be:

* trustworthy
* cacheable
* testable
* reusable
* inspectable
* useful even when no LLM is involved

### 9.1 Deterministic Tools

The deterministic tools should:

* parse source
* build semantic models
* extract symbols
* extract call summaries
* compute simple metrics
* write JSON artifacts
* cache scan results
* preserve source spans
* assign stable IDs
* support repeatable before/after comparison

Examples of deterministic facts:

```text
Function X has 9 branches.
Function Y calls A, B, and C.
Enum A and Enum B share 6 similar variant names.
Module A imports modules B, C, and D.
File X changed since the last scan.
Symbol S is public.
Function F returns Result<T, E>.
Test module M contains 12 tests.
```

The deterministic layer may detect structural signals, but those signals should remain evidence-based and traceable.

### 9.2 LLM Agents

LLM agents should:

* summarize semantic models
* interpret structural signals
* identify possible patterns
* compare summaries
* rank opportunities
* generate review briefs
* critique plans
* suggest focused follow-up questions
* generate implementation plans when requested

Example LLM output:

```text
Confirmed:
- Three command handlers contain similar validation and error-mapping shapes.

Inferred:
- These handlers may be converging on a reusable command-processing boundary.

Unknown:
- Whether the workflows share the same domain semantics.

Suggested next step:
- Compare the error-mapping spans directly before extracting a shared abstraction.
```

### 9.3 Architectural Rule

The scanner should produce facts.
The agents should produce interpretation.
The human should make design decisions.

RefactorRadar should remain useful without an LLM, but become more powerful when an LLM or MCP-capable coding agent can query its artifacts.

## 10. Scaling Model

Large codebases cannot fit into a single LLM context window, even after semantic compression.

RefactorRadar should not assume that the whole codebase can be sent to an LLM at once.

Instead, RefactorRadar should use hierarchical semantic reduction with evidence-preserving summaries.

```text
workspace
  ↓
deterministic discovery
  ↓
project/crate scan queue
  ↓
project/crate semantic models
  ↓
project/crate summaries
  ↓
cluster analyses
  ↓
workspace summary
  ↓
focused review briefs
```

The goal is not to fit the codebase into the LLM.

The goal is to produce durable intermediate artifacts that can be summarized, cached, compared, and queried.

## 11. Orchestrator and Work Queue

Future versions may include an orchestrator that coordinates scans, summaries, and analysis passes.

The orchestrator should not be the only place where state exists.

Bad design:

```text
The orchestrator remembers what every agent said.
```

Good design:

```text
Each scan, summary, and analysis pass writes structured artifacts.
The orchestrator tracks manifests, queues, dependencies, cache validity, and next work items.
```

Suggested artifact layout:

```text
.refactor-radar/
  manifest.json
  scans/
    rust-crate-a.semantic.json
    rust-crate-b.semantic.json
  summaries/
    rust-crate-a.summary.json
    rust-crate-b.summary.json
  analyses/
    error-shapes.analysis.json
    repeated-workflows.analysis.json
    test-patterns.analysis.json
  briefs/
    rust-crate-a.llm.md
    workspace.llm.md
  reports/
    workspace-radar.md
  cache/
    file-hashes.json
    scan-index.json
    symbol-index.json
```

The orchestrator should support incremental analysis:

```text
crate A changed
  ↓
rescan crate A
  ↓
regenerate crate A summary
  ↓
invalidate affected workspace analyses
  ↓
reuse unchanged crate B summary
```

## 12. Agentic Analysis Pipeline

Agentic analysis should be layered rather than one giant LLM pass.

Suggested pipeline:

```text
Pass 1: deterministic scan per crate/project
Pass 2: LLM or deterministic summary per crate/project
Pass 3: cluster related summaries/patterns
Pass 4: analyze each cluster independently
Pass 5: synthesize workspace-level report
Pass 6: generate focused evidence bundles for selected targets
Pass 7: critique implementation plans
Pass 8: compare before/after scans
```

Potential agent roles:

```text
Project Summary Agent
- summarizes one crate/project from semantic JSON

Pattern Cluster Agent
- groups similar concepts across projects

Boundary Review Agent
- reviews ownership, API, and dependency boundaries

Duplication Review Agent
- reviews repeated call sequences and shape drift

Test Review Agent
- reviews test layout, repeated setup, and missing symmetry

Synthesis Agent
- produces final ranked radar report

Plan Critique Agent
- critiques a proposed cleanup plan using focused evidence
```

Each agent should receive a narrow artifact bundle and produce a structured artifact.

## 13. Avoiding Summary Loss

Summary loss is the primary risk of hierarchical analysis.

A project summary must not be pure prose. It should preserve structured fields and evidence references.

Each summary should include:

* project/crate ID
* language
* purpose summary
* public API surface
* important symbols
* domain vocabulary
* modules/namespaces
* imports/dependency hints
* tests
* error/result shapes
* local structural signals
* candidate hints
* evidence IDs
* source span references
* unknowns

Example:

```json
{
  "project_id": "orders",
  "language": "rust",
  "purpose": "Handles order creation, update, cancellation, and order-related domain errors.",
  "public_api": {
    "types": ["Order", "OrderId", "OrderError"],
    "functions": ["create_order", "cancel_order"]
  },
  "domain_terms": ["order", "customer", "payment", "inventory"],
  "important_symbols": [
    "orders::create_order",
    "orders::cancel_order",
    "orders::OrderError"
  ],
  "local_patterns": [
    {
      "kind": "command_workflow",
      "evidence_ids": ["orders:evidence:workflow:001"]
    }
  ],
  "candidate_hints": [
    {
      "kind": "repeated_error_mapping",
      "evidence_ids": ["orders:evidence:error-map:003"]
    }
  ],
  "unknowns": [
    "Whether order cancellation and order creation should share API error wording."
  ]
}
```

## 14. High-Level Architecture

The architecture should separate core model, language scanners, analysis, reporting, CLI, and MCP.

Suggested Rust workspace layout:

```text
/
  Cargo.toml
  README.md
  LICENSE
  docs/
    adr/
      0001-refactor-radar-project-direction.md
    schemas/
      semantic-model.schema.json
      project-summary.schema.json
      evidence.schema.json
      radar-report.schema.json
  crates/
    refactor-radar-core/
    refactor-radar-rust/
    refactor-radar-report/
    refactor-radar-cli/
    refactor-radar-mcp/
    refactor-radar-analysis/
    refactor-radar-test-fixtures/
  examples/
    rust-workspace/
    csharp-solution/
  testdata/
    rust/
    csharp/
```

### 14.1 Crate Responsibilities

#### `refactor-radar-core`

Owns shared data models:

* workspace model
* project model
* file model
* symbol model
* source span model
* semantic model
* evidence model
* artifact manifests
* stable IDs
* serialization/deserialization

#### `refactor-radar-rust`

Owns Rust-specific extraction:

* tree-sitter Rust parser integration
* Rust item extraction
* modules
* `use` statements
* structs
* enums
* traits
* impls
* functions
* methods
* tests
* visibility
* return types
* call summaries
* `Result<T, E>` shapes
* source spans

#### `refactor-radar-report`

Owns rendering:

* Markdown summaries
* LLM-ready briefs
* radar reports
* focused evidence bundles
* comparison reports

#### `refactor-radar-cli`

Owns the CLI surface:

* command parsing
* file IO
* workspace discovery
* output path management
* user-facing errors

#### `refactor-radar-mcp`

Owns MCP server integration:

* tool definitions
* query handlers
* artifact loading
* focused evidence retrieval
* summary retrieval

#### `refactor-radar-analysis`

Future crate for higher-level analysis:

* repeated call sequences
* enum similarity
* naming drift
* error-shape similarity
* candidate ranking
* before/after comparison

#### `refactor-radar-test-fixtures`

Owns shared fixture projects and test helpers.

## 15. CLI Surface

The initial CLI should be small.

### 15.1 MVP Commands

```bash
refactor-radar scan <path>
refactor-radar summary <semantic-json>
refactor-radar brief <semantic-json>
```

Example:

```bash
refactor-radar scan ./crates/orders --language rust --out orders.semantic.json
refactor-radar summary orders.semantic.json --out orders.summary.md
refactor-radar brief orders.semantic.json --budget 80000 --out orders.llm.md
```

### 15.2 Near-Term Commands

```bash
refactor-radar scan-workspace <path>
refactor-radar summarize-workspace <path>
refactor-radar report <summary-json>
refactor-radar focus <candidate-id>
refactor-radar compare <before> <after>
refactor-radar mcp
```

### 15.3 Possible Final CLI Shape

```bash
refactor-radar scan .
refactor-radar summary .radar/scans/orders.semantic.json
refactor-radar brief . --budget 80000
refactor-radar report .
refactor-radar candidates .
refactor-radar focus workspace:candidate:error-mapping:001
refactor-radar compare .radar/before .radar/after
refactor-radar mcp --workspace .
```

## 16. Semantic Model

The semantic model should be compact, language-aware, stable, and serializable.

It should not attempt to be a complete compiler semantic model.

### 16.1 Core Entities

```text
Workspace
Project
Crate
Solution
File
Module
Namespace
Symbol
Type
Function
Method
Trait
Interface
Impl
Class
Struct
Record
Enum
Import
CallSummary
TestSummary
SourceSpan
Evidence
Metric
Signal
```

### 16.2 Source Span

Every important extracted entity should preserve its location.

```json
{
  "path": "crates/orders/src/create_order.rs",
  "start_line": 42,
  "start_column": 1,
  "end_line": 119,
  "end_column": 2
}
```

### 16.3 Symbol Summary

```json
{
  "id": "rust:orders:create_order",
  "language": "rust",
  "kind": "function",
  "name": "create_order",
  "qualified_name": "orders::create_order",
  "visibility": "public",
  "span": {
    "path": "crates/orders/src/create_order.rs",
    "start_line": 42,
    "end_line": 119
  },
  "signature": {
    "parameters": ["customer_id", "items", "payment_method"],
    "return_type": "Result<OrderId, CreateOrderError>"
  },
  "calls": [
    "validate_customer",
    "validate_items",
    "reserve_inventory",
    "authorize_payment",
    "persist_order"
  ],
  "metrics": {
    "line_count": 78,
    "branch_count": 5,
    "match_count": 2,
    "early_return_count": 3,
    "max_nesting_depth": 3
  },
  "domain_terms": [
    "customer",
    "item",
    "inventory",
    "payment",
    "order"
  ]
}
```

### 16.4 Evidence Record

Evidence records should preserve traceability.

```json
{
  "id": "orders:evidence:repeated-error-map:003",
  "kind": "repeated_error_mapping",
  "confidence": "high",
  "locations": [
    {
      "path": "crates/orders/src/create_order.rs",
      "start_line": 71,
      "end_line": 96
    },
    {
      "path": "crates/orders/src/update_order.rs",
      "start_line": 64,
      "end_line": 89
    }
  ],
  "confirmed": [
    "Both locations map OrderError::CustomerNotFound to HTTP 404.",
    "Both locations map OrderError::ValidationFailed to HTTP 400."
  ],
  "inferred": [
    "The mapping may belong behind a shared error conversion boundary."
  ],
  "unknown": [
    "Whether user-facing messages must differ by operation."
  ]
}
```

## 17. Rust Scanner Scope

Rust is the initial language target.

The Rust scanner should initially extract:

* crate root
* modules
* files
* `use` statements
* structs
* enums
* enum variants
* traits
* impl blocks
* functions
* methods
* visibility
* signatures
* return types
* tests
* call expressions where practical
* match counts
* branch counts
* nesting depth
* simple domain terms
* source spans

### 17.1 Rust-Specific Interest Areas

Rust-specific summary features should include:

* `Result<T, E>` return shapes
* error enum variant sets
* trait/impl relationships
* public API surface
* test module/test function inventory
* `mod` structure
* feature/module boundary hints
* repeated `match` shapes where practical

## 18. C# Roadmap

C# support is planned, but should not block the Rust MVP.

C# support may be implemented in one of two ways:

1. Rust-based tree-sitter C# scanner
2. Separate .NET-based scanner/MCP server

This decision can be made later.

C# support should eventually extract:

* solutions
* projects
* namespaces
* classes
* records
* structs
* enums
* interfaces
* methods
* constructors
* properties
* using statements
* async methods
* exception patterns
* result-like return patterns where detectable
* dependency injection constructor shapes
* test methods
* controller/handler/service patterns

C# should emit into the shared semantic model where practical, with language-specific extension fields when needed.

## 19. Reporting

The first reports should be useful without being overly ambitious.

### 19.1 Markdown Summary

A project summary should include:

```text
# Project Summary

## Overview
## Files and Modules
## Public API
## Important Types
## Important Functions
## Error and Result Shapes
## Tests
## Call Summary
## Metrics
## Review Notes
```

### 19.2 LLM Brief

An LLM brief should be explicitly designed for prompt use.

It should include:

```text
# RefactorRadar LLM Brief

## Task Framing
## Codebase Overview
## Important Symbols
## Public API Surface
## Error/Result Shapes
## Test Inventory
## Call Summaries
## Structural Signals
## Unknowns
## Suggested Follow-Up Prompts
```

The brief should include instructions such as:

```text
Separate confirmed facts from inference.
Do not recommend broad refactors without evidence.
Prefer narrow, behavior-preserving cleanup plans.
Ask for focused evidence before making architectural claims.
```

### 19.3 Radar Report

A future radar report should include refactor-relevant signals.

```text
# RefactorRadar Report

## Highest-Signal Review Targets
## Emerging Repeated Patterns
## Possible Convergence Targets
## Local Candidates
## Cross-Project Candidates
## Unknowns Requiring Human Review
## Suggested Next Steps
```

## 20. Token Budgeting

LLM briefs should support a token budget.

Example:

```bash
refactor-radar brief . --budget 80000 --out workspace.llm.md
```

The tool should reduce detail progressively when the budget is exceeded.

Suggested reduction order:

```text
raw snippets
↓
large private function details
↓
low-complexity private symbols
↓
full call lists
↓
private module details
↓
low-signal files
↓
only public API plus structural hotspots
↓
only project-level summary
```

The goal is to preserve high-signal context, not every detail.

The tool may initially use approximate token estimation rather than provider-specific tokenizers.

## 21. MCP Server Requirements

The MCP server should allow an LLM coding agent to query semantic artifacts without loading the full codebase into context.

### 21.1 Initial MCP Tools

```text
scan_project(path)
get_project_summary(project_id)
list_symbols(project_id)
get_symbol(symbol_id)
get_source_span(span_id)
generate_llm_brief(project_id, budget_tokens)
```

### 21.2 Later MCP Tools

```text
scan_workspace(path)
list_projects()
get_workspace_summary()
list_review_targets(scope)
list_cleanup_candidates(scope, confidence_min, risk_max)
get_cleanup_candidate(candidate_id)
get_evidence(evidence_id)
get_raw_spans(span_ids)
find_related_symbols(symbol_id)
find_repeated_call_sequences(project_id)
find_similar_error_shapes(project_id)
compare_scans(before_scan_id, after_scan_id)
```

### 21.3 MCP Design Rules

* Return compact structured data.
* Avoid returning large raw source by default.
* Require explicit source-span requests for raw snippets.
* Preserve IDs for follow-up queries.
* Separate facts from inference.
* Work locally without network access.
* Avoid dependence on any specific LLM provider.
* Reuse cached artifacts where possible.

## 22. Refactor Candidate Model

Refactor candidates are future-facing, but the model should be considered early so the semantic model does not block it later.

Example:

```json
{
  "id": "workspace:candidate:api-error-mapping:001",
  "title": "Normalize API error mapping across order and billing workflows",
  "category": "repeated_error_mapping",
  "scope": "cross_project",
  "confidence": "medium",
  "payoff": "medium",
  "risk": "medium",
  "affected_projects": ["orders", "billing"],
  "affected_files": [
    "crates/orders/src/create_order.rs",
    "crates/billing/src/retry_invoice.rs"
  ],
  "evidence_ids": [
    "orders:evidence:repeated-error-map:003",
    "billing:evidence:repeated-error-map:001"
  ],
  "confirmed": [
    "Both projects map domain errors into API-facing responses.",
    "Both projects contain similar status-code mapping structures."
  ],
  "inferred": [
    "A shared response-mapping boundary may reduce drift."
  ],
  "unknown": [
    "Whether billing retry errors require different user-facing semantics."
  ],
  "suggested_first_step": "Compare mappings directly and extract only mechanically equivalent status-code conversions.",
  "avoid_doing": [
    "Do not create a generic workflow abstraction.",
    "Do not merge billing retry semantics with one-shot order creation."
  ]
}
```

## 23. Future Maintainability Signals

Future analysis layers may detect:

* large functions
* deep nesting
* repeated call sequences
* similar function signatures
* similar enum variant sets
* repeated error mapping
* repeated validation flows
* repeated test setup patterns
* module dependency clusters
* naming drift candidates
* public API surface growth
* duplicate command-handler shapes
* cross-project concept drift
* repeated DTO/domain mapping
* repeated authorization checks
* repeated persistence/query mapping
* inconsistent async/concurrency patterns

These should be added incrementally and tested with fixtures.

## 24. Before/After Comparison

The comparison feature should support cleanup verification.

Example:

```bash
refactor-radar scan . --out .radar/before
# developer performs cleanup
refactor-radar scan . --out .radar/after
refactor-radar compare .radar/before .radar/after --out comparison.md
```

The comparison report should include:

* changed files
* changed symbols
* changed metrics
* removed repeated patterns
* new repeated patterns
* public API changes
* dependency changes
* new risks

Example output:

```json
{
  "candidate_id": "orders:candidate:error-mapping:003",
  "result": "improved",
  "before": {
    "repeated_error_mapping_locations": 6
  },
  "after": {
    "repeated_error_mapping_locations": 2
  },
  "new_risks": [
    "New shared mapper is public but only used inside one crate."
  ]
}
```

## 25. Testing Strategy

Testing should use small fixture projects with intentional structures.

### 25.1 Fixture Types

* Rust crate with normal modules and functions
* Rust crate with public/private API boundaries
* Rust crate with repeated error mapping
* Rust crate with repeated validation flows
* Rust workspace with duplicated concepts across crates
* Rust crate with tests and repeated test setup
* C# project with repeated service/controller patterns
* C# solution with duplicated DTO mapping
* mixed-quality examples with intentional false positives

### 25.2 Test Categories

* parser extraction tests
* semantic model tests
* source span tests
* stable ID tests
* report rendering tests
* LLM brief generation tests
* token-budget reduction tests
* cache invalidation tests
* MCP tool contract tests
* before/after comparison tests

### 25.3 Test Style

For Rust code, prefer clear, discrete tests.

Test names should describe behavior with a given/when/then shape.

Example:

```rust
#[test]
fn given_crate_with_public_function_when_scanned_then_function_has_stable_symbol_id() {
    // ...
}
```

Tests should verify evidence quality, not just output existence.

Example:

```text
Given a Rust crate with three repeated error mappings
When the crate is scanned
Then the repeated_error_mapping signal includes all three source spans
And the candidate identifies the shared variants
And the unknowns include semantic equivalence risk
```

## 26. MVP Scope

The MVP should be smaller than the long-term vision.

### 26.1 MVP Deliverables

1. Rust workspace with core crates.
2. MIT license.
3. Basic README.
4. ADR/PRD document.
5. Rust tree-sitter scanner.
6. Semantic JSON output.
7. Markdown summary output.
8. LLM brief output.
9. Source spans for extracted symbols.
10. Basic metrics:

    * line count
    * branch count where practical
    * function count
    * type count
    * test count
11. Basic call summaries where practical.
12. Fixture-based tests.

### 26.2 MVP Exclusions

The MVP does not need:

* MCP server
* C# support
* refactor candidate ranking
* before/after comparison
* git-history analysis
* LLM orchestration
* agent queue
* workspace-wide cross-project analysis
* automatic patch generation

## 27. Phase Plan

### Phase 0: Repository Bootstrap

* create repo
* add MIT license
* add README
* add ADR/PRD
* create Rust workspace
* create initial crate structure
* add test fixtures

### Phase 1: Rust Semantic Scanner

* integrate tree-sitter Rust
* scan `.rs` files
* extract files/modules/functions/types/tests
* preserve source spans
* emit semantic JSON

### Phase 2: Summary and Brief Rendering

* render Markdown summaries
* render LLM-ready briefs
* add budget-aware detail reduction
* include suggested follow-up prompts

### Phase 3: Workspace Support

* discover crates
* scan each crate independently
* write `.refactor-radar/` artifacts
* build project summary manifest
* support cached scan results

### Phase 4: Basic Structural Signals

* large functions
* repeated call summaries
* similar enum variant sets
* public API summary
* test inventory summary

### Phase 5: MCP Server

* expose artifact-backed query tools
* support focused symbol retrieval
* support source span retrieval
* support brief generation
* support Codex CLI integration

### Phase 6: Agentic Orchestration

* add scan queue
* add summary queue
* add cache invalidation
* add project summary agents
* add workspace synthesis flow
* keep artifacts inspectable and restartable

### Phase 7: C# Support

* decide Rust tree-sitter vs .NET scanner
* add C# semantic extractor
* emit shared semantic model
* add C# fixtures
* add C# summaries

### Phase 8: Refactor Candidate Reports

* repeated pattern detection
* maintainability drift signals
* evidence-backed refactor candidates
* candidate ranking
* focused evidence bundles
* before/after comparison

## 28. Open Questions

1. Should the first binary be `refactor-radar`, `rradar`, or `radar`?
2. Should artifacts live in `.refactor-radar/` or `.radar/`?
3. Should the first summaries be JSON-first with Markdown rendering, or Markdown-first with JSON metadata?
4. Should token budgeting use an approximate heuristic or provider-specific tokenizers?
5. Should MCP read only precomputed artifacts at first?
6. Should MCP be included before or after workspace support?
7. Should C# support use tree-sitter from Rust or a .NET scanner?
8. Should future agent orchestration be built into RefactorRadar or left to external tools like Codex/OpenCode?
9. Should generated summaries include raw snippets by default, or only source spans?
10. What should be the stable schema versioning strategy?

## 29. Acceptance Criteria for First Useful Version

The first useful version is successful when it can:

1. Scan a Rust crate.
2. Emit a structured semantic JSON file.
3. Render a readable Markdown summary.
4. Generate an LLM-ready brief.
5. Include source spans for major symbols.
6. Summarize public API surface.
7. Summarize tests.
8. Summarize basic function/type counts.
9. Include simple metrics.
10. Run locally without network access.
11. Be useful as input to Codex/OpenCode/ChatGPT for code review or planning.

## 30. Example End-to-End MVP Flow

A developer runs:

```bash
refactor-radar scan ./crates/orders --out orders.semantic.json
refactor-radar summary orders.semantic.json --out orders.summary.md
refactor-radar brief orders.semantic.json --budget 80000 --out orders.llm.md
```

The summary says:

```text
Project: orders

Public API:
- Order
- OrderId
- OrderError
- create_order
- cancel_order

Important modules:
- create
- update
- cancel
- error

Tests:
- 18 test functions found
- create_order has the largest test module

Review notes:
- create_order is the largest function
- OrderError is used by 5 public functions
- several functions return Result<_, OrderError>
```

The developer gives `orders.llm.md` to an LLM and asks:

```text
Review this crate summary. Identify the top areas I should inspect before adding another order workflow. Separate confirmed facts from inference and unknowns.
```

The LLM can now reason from structured context without needing the full raw crate in context.

## 31. Long-Term Vision

RefactorRadar should eventually support large codebases through artifact-backed, hierarchical analysis.

The long-term workflow may look like:

```text
scan workspace
  ↓
cache semantic models
  ↓
summarize projects
  ↓
cluster related structures
  ↓
identify review targets
  ↓
generate focused evidence bundle
  ↓
LLM generates cleanup plan
  ↓
LLM critiques cleanup plan
  ↓
human approves/edits plan
  ↓
changes are made
  ↓
before/after comparison verifies result
```

The final system should help developers and coding agents answer:

* Where is the codebase trying to converge?
* Which repeated patterns are stable enough to inspect?
* Which cleanup targets are narrow and safe?
* Which abstractions would be premature?
* What evidence supports a proposed refactor?
* What changed after the cleanup?

## 32. Final Decision Summary

We will build RefactorRadar as a local-first, MIT-licensed semantic code scanner.

The first milestone is not automated refactoring.
The first milestone is useful semantic summaries.

The architecture will separate deterministic scanning from LLM interpretation.

```text
Deterministic tools:
- parse source
- build semantic model
- extract symbols
- extract call summaries
- compute simple metrics
- write JSON artifacts
- cache scan results

LLM agents:
- summarize
- interpret
- identify patterns
- compare summaries
- rank opportunities
- generate review briefs
- critique plans
```

The project will start with Rust and grow incrementally toward MCP, workspace orchestration, C# support, maintainability signals, and evidence-backed refactor discovery.

RefactorRadar should remain useful as a standalone tool and become more powerful when paired with LLM coding agents.
