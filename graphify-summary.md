# Graphify Summary

## What It Is

Graphify is a Python package and CLI, distributed as the `graphifyy` package with
the `graphify` command. It also installs as a skill/integration for coding
assistants such as Claude Code, Codex, OpenCode, Cursor, Gemini CLI, Aider, and
others.

Its purpose is to turn a folder of code, docs, papers, images, videos, and other
project artifacts into a queryable knowledge graph.

Relevant source files:

- `submodules/graphify/pyproject.toml`
- `submodules/graphify/README.md`
- `submodules/graphify/ARCHITECTURE.md`
- `submodules/graphify/docs/how-it-works.md`

## What It Does

Graphify scans a project and writes outputs under `graphify-out/`, primarily:

- `graph.json`: the full machine-readable graph.
- `GRAPH_REPORT.md`: a human-readable report with key concepts, surprising
  connections, communities, confidence tags, and suggested questions.
- `graph.html`: an interactive browser view for searching, filtering, and
  clicking through graph nodes.

The graph captures project structure and semantic relationships, including
files, classes, functions, imports, calls, concepts, rationale, community
clusters, highly connected "god nodes", surprising cross-file connections, and
ambiguous edges that may need review.

It also exposes commands such as:

- `graphify query "<question>"`
- `graphify path "<A>" "<B>"`
- `graphify explain "<concept>"`
- `graphify export callflow-html`
- `graphify update`
- `graphify watch`

Additional integrations include MCP serving, Obsidian/wiki exports, GraphML,
Neo4j/Cypher export, PostgreSQL introspection, and optional support for PDFs,
office files, Google Workspace shortcuts, media transcription, Terraform, SQL,
and more.

## How It Works

The documented core pipeline is:

```text
detect -> extract -> build_graph -> cluster -> analyze -> report -> export
```

At a high level:

1. Graphify detects and classifies files by type: code, docs, PDFs, images,
   office files, media, SQL, Terraform, MCP configs, and related artifacts.
2. Code extraction is local and deterministic. Tree-sitter parsers extract
   classes, functions, imports, calls, inline comments, and other structural
   relationships without sending code to an LLM.
3. Non-code semantic content, such as docs, PDFs, images, and transcripts, can
   be processed through configured model backends or assistant subagents. These
   produce JSON fragments containing nodes, edges, and hyperedges.
4. AST, semantic, and optional database-schema results are merged into a
   NetworkX graph.
5. Relationships are tagged as `EXTRACTED`, `INFERRED`, or `AMBIGUOUS`, with
   confidence scores for inferred edges.
6. Communities are identified with Leiden clustering when available, with a
   NetworkX Louvain fallback.
7. The final graph and reports are written to `graphify-out/` so future queries
   can traverse a compact graph instead of rereading the full repository.

The implementation keeps the graph exchange format simple: extractors emit
plain dictionaries with `nodes` and `edges`, `build.py` converts them into a
NetworkX graph, `cluster.py` assigns communities, `analyze.py` finds notable
nodes and connections, and `export.py` writes the final artifacts.

## Practical Takeaway

Graphify is essentially a graph-based codebase and corpus indexing layer for AI
coding assistants. Its main value is orientation: instead of grepping or reading
large numbers of files repeatedly, an assistant can query the graph for scoped
context, dependency paths, important abstractions, and cross-file relationships.
