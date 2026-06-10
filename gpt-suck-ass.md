# Fix Codex Subagent Work-Plan Bootstrap Gap And Remove Broken Sampling Path

## Problem

`generate_brief_work_plan` currently assumes this flow:

```text
parent MCP session loads or scans a project
generate_brief_work_plan returns tasks using that project_id
spawned Codex subagents call get_element_brief with that project_id
```

That assumption is false. A spawned Codex subagent does not share the parent
MCP server's in-memory project cache. In the real Codex flow, both spawned
subagents failed with:

```text
requested RefactorRadar project was not found for project_id file:///home/tony/git/refactor-radar
```

The parent-only work-plan flow works because the parent session has the project
loaded. The true Codex subagent work-plan flow is broken because each subagent
must be able to bootstrap its own MCP cache before running evidence calls.

There is a second problem: the MCP sampling vector was built around
`generate_llm_brief`, but Codex does not advertise MCP sampling in this
environment. Keeping that path in the Codex-facing feature creates confusion,
failed calls, wasted validation work, and misleading status language.

## Goal

Make every generated work-plan task self-bootstrapping.

A spawned Codex subagent must be able to execute only the tool calls listed in
its task and still produce a valid brief JSON object.

Completely remove the broken MCP sampling path from this feature. The Codex
feature path is `generate_brief_work_plan` plus Codex/subagent execution of the
returned deterministic tool calls. Do not keep `generate_llm_brief` as a
Codex-facing escape hatch.

## Design

Each `generate_brief_work_plan` task must include a bootstrap tool call before
any `get_element_brief` calls.

For a project originally loaded from a SCIP file, prepend:

```json
{
  "tool_name": "load_scip_project",
  "arguments": {
    "path": "<absolute path to the SCIP file>"
  }
}
```

For a project originally produced by `scan_project`, prepend:

```json
{
  "tool_name": "scan_project",
  "arguments": {
    "path": "<absolute path to the scanned project>"
  }
}
```

The subagent then runs the existing `get_element_brief` calls.

Persist deterministic cache artifacts under a repo-root cache directory:

```text
.refactor-rador/
```

The cache should contain everything needed for a fresh Codex parent or subagent
MCP session to recreate project state without relying on another session's
in-memory cache. It must also persist generated brief and summary outputs so the
same evidence does not need to be regenerated repeatedly.

Remove the host-sampling vector:

```text
delete generate_llm_brief tool exposure
delete get_brief_generation_capabilities if it only exists to route around sampling
delete SamplingBriefLlmClient and sampling/createMessage request code
delete generated-brief cache/model-hint code that only supports host sampling
delete README/status language for sampling happy path
```

Keep only deterministic work-plan generation, evidence retrieval, and repo-local
generated-output caching produced by the Codex work-plan flow.

## Implementation Plan

### 0. Rip Out MCP Sampling

Remove the code, types, tests, and docs for the non-working host-sampling path.

Delete or stop exporting:

```text
generate_llm_brief
get_brief_generation_capabilities, unless retained as a simpler work-plan availability probe
SamplingBriefLlmClient
BriefLlmClient
BriefLlmRequest
GeneratedBrief
GeneratedBriefContent
GeneratedBriefCacheEntry
GeneratedFiveW
BriefCacheKey
LlmBriefParams
brief_llm_user_prompt
sampling/createMessage handling
model hint plumbing used only by generate_llm_brief
generated-brief cache used only by generate_llm_brief
```

Also remove MCP protocol tests and unit tests whose only purpose is validating
sampling, fake LLM clients, model-hint cache keys, invalid sampling responses,
or unsupported sampling errors.

If any of those types are still useful for deterministic work-plan output, keep
only the useful deterministic pieces and rename them so they do not imply host
sampling.

### 1. Add Repo-Root Persistent Cache

Create and use a repo-root cache directory:

```text
.refactor-rador/
```

Store deterministic artifacts there instead of relying on `/tmp` for generated
SCIP files or process-local memory for bootstrap data.

Suggested layout:

```text
.refactor-rador/
  projects/
    <project-hash>/
      project.json
      index.scip
      work-plans/
      evidence-briefs/
      generated-briefs/
      summaries/
```

`project.json` should include:

```text
project_id
project_path
scip_path
bootstrap kind
rust_analyzer_path, when relevant
created_by rr-mcp version, if available
```

The cache is a local build/runtime artifact. Add `.refactor-rador/` to
`.gitignore` unless the repository explicitly wants to commit generated indexes
and generated brief artifacts.

Cache deterministic evidence and generated brief/summary outputs:

```text
evidence-briefs/
  <symbol-hash>.<params-hash>.json
generated-briefs/
  <symbol-hash>.<evidence-hash>.<prompt-version>.<generation-context-key>.json
summaries/
  <summary-kind>.<scope-hash>.<params-hash>.json
work-plans/
  <project-hash>.<selection-hash>.<params-hash>.json
```

Generated brief cache entries must include:

```text
project_id
symbol_id or task merge_key
evidence_hash
prompt_version
generation_context_key
generated_at
generated_by, if available
source_span_ids
confirmed
inferred
unknowns
summary
five_w
```

Invalidate generated brief and summary cache entries when the SCIP/evidence
hash, prompt version, generation context key, or relevant request params change.

Do not keep the old host-sampling cache path. The cached generated output is
produced by Codex in the normal parent/subagent work-plan flow, not by `rr-mcp`
calling `sampling/createMessage`.

### 2. Track Project Bootstrap Source

Add source metadata to the cached project entry.

When `scan_project` succeeds, store:

```text
project_id
project_path
generated_scip_output_path under .refactor-rador/
bootstrap kind: scan_project
```

When `load_scip_project` succeeds, store:

```text
project_id
scip_path
bootstrap kind: load_scip_project
```

Keep this metadata inside the MCP project cache layer so
`generate_brief_work_plan` can derive the right bootstrap call from the same
loaded project it is planning against.

Also write the metadata to `.refactor-rador/projects/<project-hash>/project.json`
so a future MCP session can recover the bootstrap source.

### 3. Represent Bootstrap Calls In Work Tasks

Reuse `BriefTaskToolCall`; do not introduce a second tool-call shape unless the
existing structure cannot represent bootstrap calls.

The first `tool_calls` entry for every task should be one of:

```text
scan_project
load_scip_project
```

All evidence calls remain after the bootstrap call.

Prefer `load_scip_project` from `.refactor-rador/projects/<project-hash>/index.scip`
when a cached SCIP file exists. Use `scan_project` only when no cached SCIP file
is available or the cache is invalid.

### 4. Make Task Instructions Explicit

Update generated parent/task instructions to state:

```text
Each sub-agent must execute the bootstrap tool call first.
If the bootstrap call returns a project_id different from the task arguments,
use the returned project_id for subsequent evidence calls.
The MCP server does not spawn or manage Codex sub-agents.
```

If possible, generate evidence calls with a project identifier that remains
stable after bootstrap. If not, the instruction above is required.

### 5. Update `generate_brief_work_plan`

For each generated task:

1. Look up the cached project's bootstrap source.
2. Prefer the repo-root cached SCIP path when available.
3. Build the bootstrap `BriefTaskToolCall`.
4. Prepend it to the task's `tool_calls`.
5. Keep the existing chunking, merge key, schema, budget, dependencies, and
   acceptance criteria behavior.

If no bootstrap source is known, return a typed MCP error instead of generating
a task that cannot run in a fresh subagent.

### 6. Update Tests

Add or update unit tests for:

- `scan_project` writes generated SCIP and metadata under `.refactor-rador/`.
- `generate_brief_work_plan` prefers cached `load_scip_project` bootstrap calls.
- generated brief outputs are written under `.refactor-rador/`.
- summary outputs are written under `.refactor-rador/`.
- generated brief cache keys include evidence hash, prompt version, generation
  context key, and relevant request params.
- stale generated brief/summary cache entries are not reused after evidence or
  params change.
- `scan_project`-backed plans include `scan_project` as the first tool call.
- `load_scip_project`-backed plans include `load_scip_project` as the first
  tool call.
- Bootstrap calls appear before every `get_element_brief` call.
- Missing bootstrap metadata returns a typed error.
- No exposed `generate_llm_brief` tool remains.
- No sampling/createMessage client code remains.

Add or update MCP protocol tests for:

- A fresh MCP client can execute a returned task from bootstrap through
  `get_element_brief`.
- The task does not depend on the parent client's in-memory project cache.
- A fresh MCP client can load from `.refactor-rador/` cache.
- A fresh MCP client can reuse generated brief/summary cache entries from
  `.refactor-rador/` when the cache key is still valid.

### 7. Update README

Change the documented working Codex flow to:

```text
parent calls scan_project or load_scip_project
parent calls generate_brief_work_plan
each subagent executes the bootstrap tool call in its task
each subagent executes the returned get_element_brief calls
each subagent returns expected JSON
parent Codex merges task outputs by merge_key
```

Clarify that the bootstrap call exists because Codex subagents may not share
the parent MCP server's in-memory project cache.

Document `.refactor-rador/` as the repo-local deterministic cache directory and
state whether it should be ignored by git. Include generated brief and summary
cache behavior, including invalidation by evidence hash and generation context.

Remove README language about:

```text
host-sampled LLM briefs
generate_llm_brief
sampling happy path
Codex sampling support
unsupported-host errors
brief model hints for host sampling
```

## Acceptance Criteria

The fix is complete when all of this is true:

```text
- generate_brief_work_plan tasks are self-bootstrapping.
- Every task starts with scan_project or load_scip_project.
- scan_project stores generated SCIP/project metadata under .refactor-rador/.
- Work-plan tasks prefer load_scip_project from .refactor-rador/ when available.
- Generated brief outputs are cached under .refactor-rador/.
- Summary outputs are cached under .refactor-rador/.
- Generated-output cache keys include evidence hash, prompt version, generation context, and relevant params.
- Stale generated brief/summary cache entries are not reused.
- .refactor-rador/ is gitignored unless intentionally committed.
- A spawned Codex subagent can execute a returned task without parent MCP cache state.
- Parent-only work-plan flow still works.
- generate_llm_brief is not exposed as an MCP tool.
- Host sampling/createMessage code is removed from rr-mcp.
- Sampling-only model hint/cache code is removed.
- README states the actual working subagent flow.
- README does not present MCP sampling as a Codex feature path.
- cargo fmt --check passes.
- cargo test -p rr-mcp passes.
- cargo check -p rr-mcp --all-targets passes.
- cargo test --workspace --all-targets passes.
```
