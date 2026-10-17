# Phase 0 — documentation discovery and allowed reference APIs

Discovery date: 2026-10-10 UTC. Planning only: no upstream binary was built or run.

## Pinned source baseline

Official repository: <https://github.com/sys-bio/antimony>

Inspected `develop` commit:
`f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2`

GitHub API reported commit time `2026-09-19T01:54:07Z`. This is a **research
baseline**, not a confirmed released binary or the final compatibility target.
Choose a release with Herbert/Lucian and record its source SHA, artifacts and
dependency hashes before promoting golden results. Do not silently follow develop.

## Primary source map

| ID | Source | Read/use |
|---|---|---|
| S1 | [Tutorial](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/doc/AntimonyTutorial.md) | language, defaults, assignments, units, example models |
| S2 | [Grammar and lexer](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/antimony.ypp) | actual token spellings; expression tokens are assembled into strings |
| S3 | [SBML conversion](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/sbmlx.cpp#L249) | libSBML math-parser settings, not just Bison precedence |
| S4 | [C API declarations](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/antimony_api.h.in) | conversion API and state/memory controls |
| S5 | [Test registration](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/test/CMakeLists.txt) | API, Basic, Errors, Units, Hierarchy and other distinct suites |
| S6 | [Test helper](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/test/TestAntimonyUtil.cpp) | .txt/.xml/_rt.txt fixture conventions |
| S7 | [Reaction input](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/test/test-data/reaction.txt) and [expected XML](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/test/test-data/reaction.xml) | inspected arrow/default/unspecified-value example |
| S8 | [Python roundtrip harness](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/src/bindings/python/sbml_test_suite/test_roundtrip.py) | model conversion, solver comparison, explicit skip limitations |
| S9 | [SBML suite overview](https://sbml.org/software/sbml-test-suite/docs/overview/) and [semantic case instructions](https://github.com/sbmlteam/sbml-test-suite/blob/develop/cases/semantic/README.md) | syntax versus semantic versus stochastic tests and numerical tolerances |
| S10 | [libSBML Python document API](https://sbml.org/software/libsbml/5.18.0/docs/formatted/python-api/classlibsbml_1_1_s_b_m_l_document.html) | validation API shape; pin actual installed version separately |
| S11 | [Antimony license](https://github.com/sys-bio/antimony/blob/f3f5c6b7f3f22ab48f8e382a9318ef50bce2b4b2/LICENSE.txt) and [SBML suite repository](https://github.com/sbmlteam/sbml-test-suite) | retain applicable upstream/fixture notices; review redistribution separately |
| S12 | [Smith/Sauro 2024 paper](https://arxiv.org/html/2405.15109v1) | broader language scope and scientific context, not current release conformance |
| S13 | [quick-xml Writer](https://docs.rs/quick-xml/latest/quick_xml/struct.Writer.html) and [Winnow](https://docs.rs/winnow/latest/winnow/) | dependency candidates, not implemented APIs or pinned versions |

Line numbers in upstream discovery may move in newer sources; use these pinned
links plus symbols/sections, never transplant a guessed signature from a summary.

## Verified reference API declarations

From S4, for a development oracle, not native-library dependencies:

```c
long loadAntimonyString(const char* model);
char* getMainModuleName();
char* getSBMLString(const char* moduleName);
char* getCompSBMLString(const char* moduleName);
char* getLastError();
void clearPreviousLoads();
void freeAll();
```

`loadAntimonyString` succeeds at nonnegative indices, including zero; -1 is
failure. Flat and hierarchical output are different APIs. `clearPreviousLoads`
clears stored models; `freeAll` releases returned allocations. Do not mix custom
frees and upstream bulk cleanup. Python wrapper signatures must be inspected in
the pinned installation; do not blindly transpose pointer handling into Python.

S8 verifies Python use of `antimony.clearPreviousLoads()`, `loadSBMLFile(...)`,
`getAntimonyString()`, `loadAntimonyString(...)`, `getCompSBMLString()` and
`getLastError()`. First oracle task confirms the corresponding flat-export call
and reported versions locally before recording fixtures. Use subprocess isolation
initially; no unsupported claim about upstream thread safety is needed.

Independent SBML validation uses document `getNumErrors()`, `getError(i)` and
`checkConsistency()` with severity/category inspection, not "all messages fatal".
The presence of a document or `getModel()` result alone is not conformance.

## Findings that change implementation choices

- `->` is reversible; `=>` is irreversible. S7 demonstrates reversible=true.
- S2 lexer apparently accepts `<==>` while a source enum comment says `<=>`.
  Alternative spellings stay out of profile pending executable confirmation.
- Formula evaluation semantics come from S3/libSBML settings, including natural
  `log`, case-sensitive built-ins and unary-minus handling. Built-ins are deferred
  in our initial profile, but precedence still needs executable probes.
- `$S`/`const species S` are boundary species, not necessarily immutable values.
  Reference export sets boundaryCondition=true and constant=false in these cases.
- Missing compartments use a volume-one default compartment. Missing species or
  parameter values can remain unspecified; never interpret absence as a guessed zero.
- `=` initializes, `:=` assigns continuously, and `'=` defines a rate rule.
  Repeated definitions override in full Antimony; our profile initially rejects
  repeated initialization explicitly instead of misinterpreting it procedurally.
- Units do not automatically rescale formulas. Species generally denote
  concentrations while reaction rates are amounts/time; substance-only rules differ.
- Upstream string-based fixtures are useful sources but do not require identical
  XML byte formatting in a native implementation.
- Upstream roundtrip tests compare two runs in the same simulator and contain
  skips/limitations. They are not automatic validation against official result CSVs.

## Confidence and remaining gaps

High confidence in inspected declarations and fixture mappings. Not yet executed:
all oracle calls, operator precedence probes, alias probes, package compatibility,
unit corner cases, or the illustrative first example. No complete upstream fixture
count or claim of zero existing Rust projects is made. Check existing upstream
branches/prototypes with Herbert before duplicating work.
