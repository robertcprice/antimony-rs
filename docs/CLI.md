# Unified native CLI

**Status: implemented; the 90-case CLI compatibility gate has passed locally.** The verified language boundary is
`math-core-v0.2.1-slice`; see [current rational correction](RATIONAL_CALLS_EVIDENCE.md) and [builtin user example](BUILTINS.md). See [CLI_PLAN.md](CLI_PLAN.md) for rationale and
[the CLI review](reviews/cli-review.md) for actual executed evidence as it becomes
available. Existing example programs retain their separate command/diagnostic
protocols; this CLI does not replace the library or add language semantics.

## Build and feature boundaries

The selected binary is `antimony`, enabled by optional feature `cli`. JSON support
is an optional, exactly pinned `serde_json` dependency; default `sbml` behavior is
unchanged. Core library builds without default features must remain free of normal
third-party dependencies. No Python, simulator, network, native Antimony or libSBML
is part of CLI execution.

Build commands:

```sh
cargo build --locked --features cli --bin antimony
cargo build --locked --no-default-features --features cli --bin antimony
```

The second build supports checking/formatting but must explicitly report that
export requires `sbml`; missing export support must never look like a successful
empty export. The CLI is a thin adapter over `SourceFile`, `parse`, `analyze`,
`format_antimony` and `write_sbml`.

## Command and stream contract

```text
antimony check INPUT [--json]
antimony format INPUT [-o OUTPUT] [--json]
antimony export INPUT [-o OUTPUT] [--json]
```

`-` as INPUT means standard input. In human mode, omitted output or output `-`
means the artifact goes to standard output. Human status, warnings and diagnostics
go to standard error so they cannot corrupt Antimony or XML output.

In JSON mode, standard output contains exactly one structured report, never a raw
artifact. `format` and `export` therefore require `-o` with an actual file path;
omitting it or using `-o -` is a usage error. Checking does not create an artifact.
The JSON contract is `antimony-cli-report-v1`, specified in
[cli-report.schema.json](cli-report.schema.json). This is a CLI result schema,
not a new biological-model interchange format.

Options may occur before or after the input, before a `--` delimiter. After `--`,
arguments are literal paths; a file named `--json` must not switch report mode.
The argument after `-o` is a literal output path, even if it begins with a hyphen.
Duplicate flags/options and extra arguments are errors. `--help` and `--version`
are successful informational operations: text goes to stderr in human mode, or
one report with note diagnostics goes to stdout in JSON mode.

Examples:

```sh
target/debug/antimony check fixtures/accepted/first_conversion.ant
target/debug/antimony check fixtures/accepted/first_conversion.ant --json
target/debug/antimony format fixtures/accepted/formula_list_forward.ant -o target/formatted.ant --json
target/debug/antimony export fixtures/accepted/first_conversion.ant -o target/model.xml --json
printf 'formula a=1,b=a+2;\n' | target/debug/antimony format -
target/debug/antimony check -- --json
```

### Report fields

All schema fields are present; unavailable optional values are `null`. Consumers
should check schema version, status and process exit together, not infer success
from a model summary or the presence of a JSON object.

| Field | Meaning |
|---|---|
| `schema` | Exactly `antimony-cli-report-v1` |
| `command` | `check`, `format`, `export`, or null before a valid command is known |
| `status` | `success`, `rejected`, or `error` |
| `exit_code` | 0, 2, or 1 respectively |
| `profile_id` | Actual implemented language subset, not the broader release target |
| `source` | Null or `{display, lossy}`; display spelling is not a reversible OS-path encoding |
| `model` | Null or checked identity and counts for compartments, species, parameters, reactions, InitialAssignments and unit definitions |
| `output` | Null or `{destination, path, written}`; destination is `stdout`/`file`; a path uses `{display, lossy}` |
| `diagnostics` | Ordered structured errors, warnings and notes; never silently discarded |

Each diagnostic has `stage`, `severity`, `code`, `message`, `labels` and `notes`.
Stages are usage/input/syntax/resolve/format/export/output. Severity is
error/warning/note, with `unknown` reserved for an unmapped future core variant.
Each label preserves primary/secondary style, source ID, `start_byte`, `end_byte`,
and nullable `start`/`end` locations containing `line`, `column`, `byte_column`.
Messages/notes are JSON-escaped strings; terminal control characters in human
output must also be escaped. Raw successful artifact bytes are not escaped.

Usage errors generally have no source/model/output. Once valid arguments identify
an operation, source and intended output remain visible on rejection, with
`written:false`. Model information can exist after successful analysis even when
formatting or file output later fails; it is not proof of overall success.
Export without the SBML feature fails before opening input. A final output-report
write failure after artifact commit returns failure, but cannot undo that commit.

## Exit codes and diagnostics

| Exit | Meaning |
|---|---|
| 0 | Requested operation succeeded; warnings may still be present |
| 1 | Usage, input/output or unavailable-feature error |
| 2 | Language/semantic/profile/resource rejection |

A rejected operation is not a successful translation. Warnings remain visible in
both human and JSON output. Report diagnostic codes are stable families; wording
is not the compatibility contract. Distinguish parser rejection, semantic
resolution and artifact production stages; do not call valid excluded syntax
malformed merely because the CLI cannot support it.

Diagnostic byte ranges are zero-based and half-open, referring to original UTF-8
source. Human/JSON line and column positions are one-based. `column` counts Unicode
scalar values, **not** display cells, grapheme clusters, UTF-16 units or expanded
tabs; `byte_column` counts UTF-8 bytes. CRLF, LF, lone CR and EOF use existing
`SourceFile::location`. Missing source text or source-size rejection may yield
no labels; the CLI must not fabricate positions. Display paths are labels and may
be lossy representations of OS paths; the report must make that lossiness explicit.

## File safety and limits

Input reads are bounded before parsing. Explicit paths are accessed only by the
CLI adapter; source import statements remain unsupported and are never followed.
A source name is not executed, interpreted as a shell command or used as an
implicit path to another input.

For file output, fully validate/serialize first, reserve a new temporary file in
the destination directory, write/flush it, then rename it into place. Validation
or pre-commit write failure must preserve existing output; failed writes must
clean temporary files. Format and export share this contract. Successful in-place
formatting is permitted only after the complete input has been read.

This is not a durability/fsync, crash-rollback, multi-file transaction or hostile
filesystem sandbox guarantee. The selected policy rejects final output symlinks, nonregular destinations and
read-only files; preserves regular-file permission bits; and uses private Unix
staging files (0600). Replacement detaches a hardlink rather than modifying other
names for the original inode. Ownership, ACLs and extended attributes are not
preserved. Nonregular file inputs are rejected; explicit stdin remains allowed.

This local macOS filesystem rejected attempts to create invalid-UTF-8 filenames
with OS error 92 before the adapter could open them. Tests must report that
platform limitation, not claim arbitrary Unix non-UTF-8 path operation succeeded.
Human/JSON display strings still explicitly record path lossiness. Local macOS
verification cannot establish Linux/Windows replacement behavior.

## Validation contract

1. Compare all 90 original fixtures across library, existing examples and CLI.
   Expect 48 supported cases and 42 required rejections, separately counted.
2. Require CLI-format bytes to match the native formatter and be byte-idempotent;
   inspect CLI-export SBML with the unchanged independent libSBML projection and
   live pinned upstream golden checks.
3. Compare rejection stage/code and warning codes to the existing pipeline; test
   JSON types, schema version, status/exit consistency, notes and source labels.
4. Exercise malformed arguments, stdin/stdout, JSON/artifact separation, missing
   inputs, invalid UTF-8, limits, in-place paths, Unicode/non-UTF-8 paths, existing
   output sentinels and output failures. No partial output earns success.
5. Exercise full/default/no-default/CLI-only feature builds. Keep existing native,
   reference and formatter evidence current after any executable hash changes.
6. Missing reference tools or prebuilt executables must be blocked, not passed.
   Numerical checks and remote CI remain NOT_RUN unless actually executed.

## Independent checker invocation

The optional development checker never builds binaries or installs tools. After
explicitly building trusted executables, the gate invocation is:

```sh
.venv-oracle/bin/python tools/oracle/cli_check.py \
  --cli target/debug/antimony \
  --native target/debug/examples/export_sbml \
  --formatter target/debug/examples/format_antimony --trusted-native
```

It freezes hashes of all three programs, the fixture manifest, profile, report
schema and checker files; changed inputs fail the run. It requires live reference
agreement before granting CLI corpus credit. The original corpus expectation and
semantic projection are unchanged. Missing programs or oracle dependencies are
blocked, not silently skipped. The checker uses a deliberately bounded stdlib
validator for this report schema, not a general JSON-Schema engine.

Schema vocabulary was checked against the official [JSON Schema 2020-12 Core](https://json-schema.org/draft/2020-12/json-schema-core)
(`$ref`, `$defs`, `anyOf`, object/array applicators) and
[Validation vocabulary](https://json-schema.org/draft/2020-12/json-schema-validation)
(`type`, `enum`, `const`, `minimum`, `minLength`, `required`). The local checker
recognizes only the keywords actually used here and rejects unexpected schema
vocabulary. It additionally requires integer JSON wire values as emitted by the
CLI's unsigned counts, rather than accepting all mathematically integral floating
representations permitted by general JSON Schema. This stricter report-protocol
check is not a claim of general Draft 2020-12 validator conformance.
