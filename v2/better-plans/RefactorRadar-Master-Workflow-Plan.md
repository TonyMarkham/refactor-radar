# RefactorRadar Master Workflow Plan

This plan keeps the whole RefactorRadar process in one place so the product
workflow stays coherent. The organizing principle is not crate creation, file
layout, or an internal refactor sequence. The organizing principle is the
Codex-facing workflow: Codex asks RefactorRadar to document a target, the MCP
routes produce and audit a SCIP artifact, the RefactorRadar Skill orchestrates
parallel summarization work, and the artifact database is updated from the
structured outputs of that work.

The end product is a Codex-native documentation pipeline that avoids
resummarizing unchanged code. RefactorRadar scans source into SCIP, decodes that
SCIP into Rust objects, audits the decoded index against the artifact database,
returns a structured manifest of only the new or changed documentation work,
lets the Codex RefactorRadar Skill parallelize that work through sub-agents,
and records the resulting artifact database objects.

## Workflow Rubric

Every implementation choice should be tested against this workflow:

```text
User asks Codex to document a Rust crate or C# project.
Codex uses the RefactorRadar Skill.
The Skill calls rr-mcp scan.
rr-mcp runs the configured SCIP scanner.
The scanner writes a .scip artifact under .RefactorRadar/.
The Skill or rr-mcp passes that artifact path to the decode route.
The decode route returns or internally retains an rr_scip::Index.
The audit route tests that index against the artifact database.
The audit route returns a sub-agent manifest for only changed work.
The Skill starts summarizer sub-agents from that manifest.
Each sub-agent writes its summary artifact under .RefactorRadar/.
Each sub-agent returns a structured artifact database object.
The Skill applies those structured objects to update the artifact database.
```

If a proposed implementation does not serve this route chain, it is probably
out of scope. If a proposed split makes it unclear where the scanned SCIP
artifact is decoded, where it is audited, or how the Skill gets the sub-agent
manifest, the split is wrong.

## Product Boundary

RefactorRadar is a Codex workflow tool. Its primary consumer is the Codex
RefactorRadar Skill, not a human CLI session and not a standalone report
generator. The Skill owns orchestration. The MCP server owns scanner invocation,
SCIP artifact handling, decoding, and audit routes. The artifact database is the
source of truth for what has already been summarized.

The key unit of work is not a crate, file, or raw symbol by itself. The key unit
of work is a documentation artifact candidate discovered from the decoded SCIP
index and compared against the artifact database. The audit route should return
only the candidates that require sub-agent work.

## MCP Route Contract

The first required MCP route is `scan`. It accepts enough information to choose
and run the configured scanner for the requested target. For Rust, that means
running `rust-analyzer scip`. For C#, that means running `scip-dotnet index`.
Scanner commands are configured outside the plan in the user-facing
configuration. The route writes the generated `.scip` file under
`.RefactorRadar/` and returns the path to that artifact.

The second route is `decode`. It accepts the path to a `.scip` artifact and
decodes it into Rust SCIP objects. The decoded representation is
`rr_scip::Index`. This route exists so the product contract is explicit:
RefactorRadar audits a decoded SCIP index, not an invented intermediate that
obscures where SCIP entered the system. The implementation may choose whether
to return a compact handle, cache the decoded index for the following route, or
immediately pipe the decoded value into audit, but the route boundary must
remain conceptually clear.

The third route is `audit`. It accepts the decoded SCIP index or a handle to it,
loads the artifact database, and compares the current scan against the recorded
artifact state. It discards unchanged files and unchanged artifact candidates.
It retains new, changed, stale, missing, and deleted artifact-relevant work. Its
output is a structured sub-agent manifest that the RefactorRadar Skill can use
directly.

The fourth required capability is artifact database update from completed
sub-agent work. In the intended workflow, each sub-agent returns a structured
artifact database object after writing its summary artifact under
`.RefactorRadar/`. The RefactorRadar Skill collects those objects and applies
them to the artifact database. If this update is exposed through MCP, the route
should validate the structured objects and write the database. If the Skill
writes the database directly, the object shape still needs to be stable,
validated, and deterministic.

## Scanner Configuration

Scanner choice is configuration-driven. Codex should not hard-code scanner
commands into the Skill logic. For a Rust target, the configured command should
produce a SCIP artifact using `rust-analyzer scip`. For a C# target, the
configured command should produce a SCIP artifact using `scip-dotnet index`.

The scan route should return the saved artifact path, not a fully audited
manifest. Keeping scan separate from audit preserves a concrete artifact that
can be inspected, decoded again, cached, or used to debug the workflow.

## Decoded SCIP Audit

The audit route is the core of RefactorRadar. It reviews the decoded
`rr_scip::Index` against the artifact database and determines what work is
necessary. It should not ask Codex to reason over the whole scan. It should
return a compact manifest containing only the work that requires summarization
or artifact expiration.

The audit should use file-level gates first. If a scanned document is unchanged
relative to the artifact database, all artifact candidates rooted in that file
should be discarded from the manifest. If a file is new or changed, the audit
should inspect the artifact-relevant entries from the decoded SCIP index and
compare them to existing artifact records.

Element or artifact candidate comparison should be deterministic. Source ranges
from SCIP are locators for the current scan, not durable identity by themselves.
The audit should avoid silently using line numbers as permanent identity. When
identity is ambiguous, it should produce a diagnostic rather than inventing a
fragile fallback.

The audit result should include enough information for the Skill to launch
sub-agents without loading the full repository into the orchestrator context.
The manifest should include stable work IDs, target artifact paths, source
locations or evidence handles, display names, language/kind evidence, previous
artifact status when relevant, and the expected structured output shape for the
sub-agent.

## Sub-Agent Manifest

The manifest is the handoff from RefactorRadar to Codex orchestration. It is not
prose instructions and it is not a raw dump of SCIP. It is structured work.

Each manifest item should identify one documentation artifact task. It should
tell the Skill which source evidence is needed, where the sub-agent should write
the summary artifact, and what structured artifact database object the
sub-agent must return. The manifest should be deterministic enough that a run
can be resumed after a Codex context change.

The manifest should not include full source files unless there is a deliberate
reason. The Skill and sub-agents should fetch or receive scoped source evidence
for each task. This keeps the orchestrator context small and makes parallel
summarization practical.

## Codex Sub-Agent Spawn Contract

The RefactorRadar Skill should treat the audit manifest as scheduler input for
Codex sub-agents. In current Codex sessions, the relevant sub-agent tool shape is
`multi_agent_v1.spawn_agent`. The Skill should call it once per manifest item,
or once per small compatible batch when batching is deliberately chosen.

Directional tool call shape:

```json
{
  "agent_type": "worker",
  "fork_context": false,
  "items": [
    {
      "type": "text",
      "text": "{one manifest work item plus the summarization rubric}"
    }
  ],
  "message": "{plain-text fallback prompt for the same task}"
}
```

The Skill should omit model, reasoning, and service-tier overrides unless the
user or Skill configuration explicitly asks for them. The sub-agent task prompt
should tell the worker that it owns only the assigned work item, must not
rediscover the work queue, must write the assigned artifact path, and must
return the expected structured object.

Minimum directional manifest shape:

```json
{
  "manifest_id": "rr-manifest-...",
  "run_id": "rr-run-...",
  "scip_artifact_path": ".RefactorRadar/runs/rr-run-.../scan/index.scip",
  "artifact_db_path": ".RefactorRadar/artifacts.json",
  "work_items": [
    {
      "work_item_id": "wi-...",
      "task_kind": "summarize_source_artifact",
      "target_artifact_path": ".RefactorRadar/runs/rr-run-.../artifacts/wi-....json",
      "evidence": {
        "document_path": "src/lib.rs",
        "source_slice": {
          "start_line": 10,
          "start_character": 0,
          "end_line": 42,
          "end_character": 1
        },
        "mcp_tool": "get_source_slice"
      },
      "task_details": {
        "language": "rust",
        "display_name": "public_sum",
        "scip_symbol": "rust-analyzer cargo fixture 0.1.0 fixture/public_sum().",
        "symbol_kind": "Function",
        "change_reason": "new"
      },
      "expected_output": {
        "schema": "artifact_db_update_v1",
        "required_fields": [
          "work_item_id",
          "target_artifact_path",
          "artifact_db_object"
        ]
      }
    }
  ]
}
```

The fixed orchestration fields are `work_item_id`, `task_kind`,
`target_artifact_path`, `evidence`, `task_details`, and `expected_output`.
`task_details` is the deliberate extension point for the actual task payload:
language-specific SCIP evidence, display labels, summary aspect, prior artifact
state, change reason, or extra instructions needed by that one sub-agent. The
Skill should not have to understand every task-specific field to schedule the
work; it only needs enough stable fields to spawn the agent, assign the output
path, and validate that the returned object matches the expected output shape.

## Local Codex Repo Structure

All Codex workflow artifacts for RefactorRadar should live in this repository.
Do not rely on global user skills or global user agents for the product
workflow. The repo should carry the Skill, the summarizer agent definition, and
the MCP configuration needed to run the workflow from a fresh checkout.

Planned local repo structure:

```text
.agents/
  skills/
    refactor-radar/
      SKILL.md
      references/
        workflow-rubric.md
        manifest-schema.md
        artifact-db-update-schema.md
      templates/
        summarizer-task.md
.codex/
  agents/
    refactor-radar-summarizer.toml
  config.toml
```

`.agents/skills/refactor-radar/` is the repo-local orchestrator Skill package.
`.codex/agents/refactor-radar-summarizer.toml` is the repo-local custom
sub-agent definition. `.codex/config.toml` is the repo-local Codex
configuration, including the RefactorRadar MCP server wiring.

## Codex Skill And Agent Definitions

The workflow needs two Codex-side artifacts in addition to the MCP server:

```text
.agents/skills/refactor-radar/SKILL.md
.codex/agents/refactor-radar-summarizer.toml
```

The repo-local Skill is the orchestrator. Codex skills are directories with a
`SKILL.md` file, and this project should keep the RefactorRadar Skill under
`.agents/skills/refactor-radar/`. The Skill should be the durable workflow
entrypoint Codex uses when the user asks to document, refresh, or summarize a
Rust crate or C# project through RefactorRadar.

Directional Skill shape:

```text
.agents/skills/refactor-radar/
  SKILL.md
  references/
    workflow-rubric.md
    manifest-schema.md
    artifact-db-update-schema.md
  templates/
    summarizer-task.md
```

Minimum `SKILL.md` contents:

```markdown
---
name: refactor-radar
description: Use when the user asks Codex to document, summarize, refresh, or audit documentation for a Rust crate or C# project using RefactorRadar.
---

# RefactorRadar Skill

Use rr-mcp to scan the requested target, decode the saved SCIP artifact, audit
the decoded index against the artifact database, and receive a manifest of only
new or changed summarization work.

Treat the audit manifest as the source of truth. Do not rediscover changed
files by reading the repository yourself. Do not spawn summarizer agents for
unchanged work.

For each manifest work item, spawn one summarizer sub-agent or one deliberate
small batch. Pass the work item, the assigned artifact path, the evidence
instructions, and the expected output schema. Collect the structured outputs
and apply them to the artifact database through the planned update path.
```

The Skill should also define the prompt template used for each sub-agent. The
template should include the work item JSON, the summarization rubric, the
allowed evidence tools or source slice instructions, the assigned output path,
and the exact structured output schema the sub-agent must return.

The summarizer agent is the execution-focused worker for one manifest item. For
this project, keep the custom agent definition under `.codex/agents`.
Standalone custom agent files must define `name`, `description`, and
`developer_instructions`; optional fields such as model, reasoning effort,
sandbox, MCP servers, and skill config should be omitted unless the workflow
needs an explicit override.

Directional custom agent file:

```toml
name = "refactor-radar-summarizer"
description = "Summarizes one RefactorRadar manifest work item and returns the structured artifact database update object."
developer_instructions = """
You execute exactly one RefactorRadar summarization task, or one explicit small
batch if the parent Skill assigns a batch.

Use only the manifest item and scoped evidence provided by the parent Skill or
the allowed RefactorRadar MCP evidence tools. Do not scan the repository, do not
audit the artifact database, do not decide whether the work is stale, and do not
edit source files.

Write the summary artifact to the assigned target_artifact_path. Return only the
structured artifact database update object requested by expected_output. If the
task cannot be completed, return a structured failed status with diagnostics
instead of inventing missing evidence.
"""
nickname_candidates = ["RR Summary A", "RR Summary B", "RR Summary C"]
```

The implementation can initially spawn the built-in `worker` agent with the
summarizer instructions embedded in the task prompt, because the currently
available session tool exposes `default`, `explorer`, and `worker`. Once the
repo-local custom agent is available to Codex in this project, the Skill should
prefer the named `refactor-radar-summarizer` agent for manifest work items. The
plan must keep both paths compatible: the manifest and prompt template should
be complete enough for a generic worker, while the custom agent should make the
same behavior reusable and less error-prone.

## RefactorRadar Skill Responsibilities

The RefactorRadar Skill is responsible for the user-facing Codex workflow. It
calls `scan`, calls `decode` or the equivalent decode-plus-audit sequence, calls
`audit`, receives the manifest, starts sub-agents, collects structured outputs,
and updates the artifact database.

The Skill should treat the manifest as the source of work. It should not
rediscover changed files by reading the repository itself. It should not
resummarize unchanged artifact candidates. It should not ask sub-agents to
decide whether their work is necessary; the audit route already made that
decision.

Each sub-agent task should be narrow. A sub-agent gets one manifest item or a
small compatible batch, reads the required evidence, writes the summary artifact
to the assigned `.RefactorRadar/` path, and returns the structured artifact
database object requested by the manifest. The Skill collects those outputs and
applies them.

## Artifact Database

The artifact database records the documentation state known to RefactorRadar.
It should be durable, deterministic, and local to the repository. It records
which scanned source artifacts have already been summarized, what source state
those summaries correspond to, where the summary artifacts live, and which
records are current, stale, missing, failed, or deleted.

The artifact database should be updated only from structured data. Summary
prose alone is not enough. A completed sub-agent task must return the database
object or update object that proves what source item was summarized, what
artifact path was written, and what source or artifact hash the update
corresponds to.

Deleted source files or deleted artifact candidates should be represented in
the audit result so the database can expire stale entries. Deleted entries do
not create summarization work, but they do affect artifact state.

## Implementation Order

The implementation should follow the workflow order, keeping the product route
chain visible at every step.

First, define the artifact database shape, manifest shape, and sub-agent output
shape in enough detail that `audit` and the Skill can agree on structured data.
Do this before building broad scanner abstractions, because the manifest is the
actual handoff that makes RefactorRadar useful to Codex.

Second, implement `scan` for Rust and preserve the saved `.scip` artifact path
as the route output. C# can follow the same route contract once its scanner
configuration is available.

Third, implement `decode` over the saved `.scip` artifact and make the decoded
SCIP index the explicit input to audit.

Fourth, implement `audit` against a small artifact database fixture. Prove that
unchanged files are discarded, changed files produce manifest items, deleted
entries are represented, and ambiguous identity produces diagnostics instead of
unstable output.

Fifth, define the repo-local RefactorRadar Skill and summarizer agent contract.
The Skill should live under `.agents/skills/refactor-radar/`, and the planned
custom summarizer agent should live under `.codex/agents/`. The first working
version may spawn built-in `worker` agents with the summarizer instructions
embedded in the task prompt, but the Skill and prompt template should already
match the named summarizer agent contract.

Sixth, implement the Skill orchestration contract using the manifest. The Skill
should be able to start sub-agents from manifest items and collect structured
artifact database objects.

Seventh, implement the artifact database update path from those structured
objects. The update should validate paths, source state, artifact state, and
database consistency before writing.

## Verification Rubric

The workflow is not complete until it can demonstrate an end-to-end run:

```text
scan target -> .scip path
.scip path -> decoded rr_scip::Index
decoded index + artifact DB -> manifest
repo Skill + summarizer prompt/agent contract are available
manifest -> parallel sub-agent tasks
sub-agent outputs -> artifact DB update
second run with unchanged source -> no summarization work
changed source -> only affected manifest items return
deleted source -> stale artifact records are expired
```

Every test and manual verification should map back to one of those arrows. If a
test only proves an internal helper but does not protect the route workflow, it
is secondary.

## Non-Goals

This plan does not require a standalone reporting UI. It does not require an
editor integration. It does not require Codex to read full SCIP indexes into
prompt context. It does not require sub-agents to decide whether work is stale.
It does not require the scanner route to produce summaries. It does not require
audit to write summary prose.

The product succeeds when Codex can ask RefactorRadar to document a target,
RefactorRadar can return only the necessary structured summarization work, and
the Skill can update the artifact database from completed sub-agent outputs.
