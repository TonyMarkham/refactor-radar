## Hetzner v4 docs
```text
read:
- .docs/Hetzner/v3/ (Reference Only, do not edit)
- .docs/Hetzner/v4/

Document exact Hetzner VPS configuration commands.
Do not create arch/INFRA-VS-* plan chains for this work.
Prefer short runbooks over planning prose.
```

---

## Init
```text
read:
- docs/adr/0001-refactor-radar-project-direction.md
- VS-llm-brief.md
- gpt-suck-ass.md

turn `gpt-suck-ass.md` into a real impl plan with actual bash/code snippets

**CRITICAL** Be sure to use all rust coding directives found in AGENTS.md
**CRITICAL** Follow ALL current error handling practices in this repo.
**CRITICAL** Be sure to only have 1 type (enum/struct/etc) per file.
**CRITICAL** YOU ARE ONLY APPROVED TO EDIT THE PLAN FILE! DO NOT CREATE/EDIT/DELETE ANY CODE FILES!
**CRITICAL** DO NOT ALTER ANY SUBMODULE.
**CRITICAL** EVERY aspect of the plan NEEDS implementation code snippets. Prose-Only is UNACCEPTABLE!
```

---

## Guided Implement
```text
$online-entity-codex-plugin:guided-implement gpt-suck-ass.md

- YOU should run the verification steps in the plan
- YOU should write/edit the tests in the plan
- **CRITICAL** YOU never write/edit Source Code unless I explicitly give you permission
- When presenting edit steps, always provide the target line number as a landmark
```

---

## Confirm
```text
fmt, check, clippy, build and test all clean
```

---

## Pre-Commit
```text
perform your pre-commit checks
```
---

## Commit
```text
I have staged everything I care about, commit without a byline
```

---


## `optimize-plan` call
```text
/online-entity-cc-plugin:optimize-plan VS-180.md
**NOTE** Assume that cargo check/clippy/build/test are all clean
**NOTE** Assume that dotnet build/test/publish are all clean
```

---

## Split
```text
- Read `VS-180.md`
- Audit it against the current repository code
- Split into 3 plans:
  VS-171.md: database migrations + Rust
  VS-172.md: C#
  VS-173.md: Infra

- **CRITICAL** Be diligent not to miss anything
```

---

## Audit
```text
- Read `VS-172.md`
- Audit it against the current repository code
```

---

## Init
```text
- I don't want you to write any code in any files.
- I want you to present me with all code and I will implement it myself, refactoring to my style.
- Present aspects of the plan 1 step at a time, the intent being to be able to maintain cognitive focus on the step
- Always read any existing file before presenting any edits to it to be sure you have the latest context
- When presenting code to me, include the path of the target file relative to the repo root
- Prefer a `Find` and `Replace` strategy when instructing an edit to an existing file
- For adding new elements to an existing file, provide un-altered `Insert After`, `Insert This` and `Insert Before` landmarks to make it obvious where you intend the addition to be inserted
```

---

## Doc
```text
- Review all MD's in `.docs/Hetzner/` (go no deeper)
- Review `.docs/__upload_to_hetzner__.md`
- Update them all with the new info without changing the style of the doc.
- Identify what aspects of the plan have no thematically obvious doc to be injected into.
```

---

## Commit
```text
I have staged everything I care about, commit without a byline
```

---

---

---

---

---

## Audit
```text
- Read `AUDIT-01-SECURITY.md`
- Audit it against the current repository code
- Develop a plan to implement production-grade solutions for each issues
```

---

## Optimize
```text
/online-entity-cc-plugin:optimize-plan AUDIT-01-SECURITY.md opus
**NOTE** Assume that cargo check/clippy/build/test are all clean
**NOTE** Assume that dotnet build/test/publish are all clean
```

---

---

---

---

---

## Soul Audit
```text
audit this repo and identify all edges related to `data/platform/config.toml`
```

```text
❯ use the soul mcp to:
  1. Add markdown Docs
  2. Use the `soul-attributes` cargo crate to annotate rust code related to the new docs
  3. Use the `Soul.Attributes` nuget package to annotate C# code related to the new docs

**NOTE** Use the docs in `.docs/soul/vault/` as a template for how to structure this work
```
