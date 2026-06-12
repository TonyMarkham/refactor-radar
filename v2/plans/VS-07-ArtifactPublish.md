# VS-07: Artifact Publish

## Objective

Register completed summary artifacts and update the artifact DB after
summarizer subagents finish.

This is the end-of-run step. It is not part of scanner process invocation and
not part of subagent orchestration.

## Depends On

- VS-01 `rr-data-model`
- VS-04 artifact DB shape and hash gates
- VS-06 work manifest and receipts

## Deliverables

- Add receipt validation.
- Add artifact file hash validation.
- Add artifact DB update logic.
- Add stale artifact expiration for deleted files/elements.
- Add publish/finalize MCP tool.
- Add tests.

## Publish Inputs

The publish step should consume:

- `run_id`
- `work-manifest.json`
- summarizer receipts
- existing artifact DB
- scan-run metadata

It should not require Codex to read all artifact contents into context.

## Publish Behavior

For each receipt:

1. Confirm `work_item_id` exists in the manifest.
2. Confirm `artifact_path` matches the preassigned path.
3. Read the artifact file locally.
4. Compute artifact SHA-256.
5. Compare it to receipt `artifact_sha256`.
6. Validate required artifact metadata.
7. Update the matching artifact DB aspect record.

After receipt processing:

1. Mark missing expected receipts as incomplete.
2. Expire artifacts for deleted files/elements when scan metadata proves they
   are gone.
3. Write the updated artifact DB atomically.
4. Persist publish diagnostics.

## MCP Shape

Recommended tool:

```text
publish_summary_run(run_id, receipts) -> PublishSummaryRunResult
```

Result should include:

- run ID
- accepted count
- rejected count
- incomplete count
- expired count
- artifact DB path
- diagnostics

## Artifact DB Updates

Each accepted artifact should update:

- aspect hash
- artifact path
- artifact SHA-256
- source content hash used for the summary
- run ID
- timestamp if the model already uses one

Do not update element content hashes from summary artifacts. Element hashes come
from scanner/source state.

## Error Handling

Errors should cover:

- missing run
- missing manifest
- malformed receipt
- unknown work item ID
- artifact path mismatch
- artifact file missing
- artifact hash mismatch
- artifact schema mismatch
- artifact DB write failure

Use the repo typed error policy.

## Tests

Add tests for:

- valid receipt updates artifact DB
- unknown work item is rejected
- wrong artifact path is rejected
- hash mismatch is rejected
- missing artifact file is rejected
- incomplete run reports missing receipts
- deleted file metadata expires stale artifacts
- artifact DB write is atomic or safely staged

## Verification

Run:

```bash
cargo test -p rr-core
cargo test -p rr-mcp
```

Manual verification:

1. Run a scan that creates work items.
2. Write one valid fake artifact.
3. Publish a matching receipt.
4. Confirm the artifact DB records the artifact path and hash.
5. Publish with a bad hash.
6. Confirm the publish step rejects it.

## Non-Goals

- Do not spawn summarizer subagents.
- Do not judge summary quality.
- Do not rescan source code.
- Do not mutate source files.
