# release-train-dogfood

An end-to-end harness that drives the **synchronizer release-train library**
(consumed as a pinned git dependency,
`github.com/LiGoldragon/synchronizer@dfae1fda`) against the **real six-crate
language-family stack** on GitHub, and records — from a genuine caller's seat —
exactly where the documented CLI and skill would strand a user versus what the
library actually requires.

It produced the first **non-synthetic** release-train closure:

- closure identity `42df158c9708d7f06c980a9431a51c4952d92560d9bfa9ce27de45b9288e6cea`
- six real `train/language-family-slice-three` candidate branches pushed to the
  six Claude-owned crates
- six real narHashes and six real per-component Cargo/flake lock identities
- a generated `release-train.lock.json` + integration `flake.nix`
  (`integration/`)

The harness is READ-ONLY on the synchronizer repository. It only pushes
`train/<name>` candidate branches to the six member crates; it never writes any
`main`, never merges, and never edits the language producers (`nota`,
`schema-*`, `sema-engine`, `spirit`).

## What it drives

The train intent is authored as data at
`/home/li/primary/release-trains/language-family-slice-three.nota` (mirrored in
`tests/intent.rs`): the six members
(`content-identity`, `name-table`, `raw-discovery`, `structural-codec`,
`core-schema`, `structural-codec-derive`) as `Mainline` selectors with their
exact observed `main` tips as expected bases, and zero immutable externals
(none of the six depends on `nota`/`schema-*`; every internal edge is inside the
member set and every other dependency is out-of-scope crates.io or
`LiGoldragon/rust-build`).

`DogfoodInvocation::run` drives the library chain stage by stage, capturing the
evidence in `run-evidence.txt`:

1. decode the intent (typed `ReleaseTrainIntent`)
2. `ReleaseTrainRun::from_config(config, intent).execute()` against the real
   remotes — resolve selectors, check bases, push the six `train/<name>`
   candidate branches, then run the ordinary StagedCascade
3. real dependency discovery via `DependencyGraph::discover`
4. real narHash attestation via `nix flake prefetch` + real per-component lock
   identities
5. `MaterializedReleaseTrain::resolve_closure` → the typed
   `ResolvedReleaseTrain`
6. `ResolvedReleaseTrain::write_integration_artifacts` → the portable
   `release-train.lock.json` + integration flake
7. Nix-level evaluation of the generated flake (below)

Run it: `release-train-dogfood [intent.nota] [checkout-root] [output-dir]`.

## Stage results (this run)

- **b — execute:** six candidate branches pushed. Cascade reported every
  component `AlreadyAligned`, so each candidate is a pure empty *materialize*
  commit on the component's selected `main` tree (each candidate narHash equals
  the corresponding `main` narHash). With an all-`Mainline` train where every
  consumer already pins every producer's current `main`, the resolver targets
  each producer at its `main` tip — which the consumers already pin — so no lock
  rewrite fires and the candidates do not cross-pin each other's candidate
  commits. Verification was `NotAttempted` by design (builder-host resolution
  pointed at an absent cluster proposal, so no ssh / `nix build`).
- **c — discovery:** real. `DependencyGraph::discover` found 6 internal
  components and 21 Cargo edges forming the DAG
  `content-identity, raw-discovery → name-table → structural-codec → core-schema
  → structural-codec-derive`.
- **d — attestation:** real. Six narHashes from `nix flake prefetch`; six lock
  identities (blake3 over each candidate's real `Cargo.lock`/`flake.lock`).
- **e — closure:** `resolve_closure` returned a typed `ResolvedReleaseTrain`,
  identity `42df158c…`.
- **f — artifacts:** `integration/release-train.lock.json` +
  `integration/flake.nix`, zero `path:` references.
- **g — Nix proof:** see the ledger's defect 3.

## Friction ledger — where the documented path strands a user

Each item maps to the audit `reports/synchronizer/train-flow-audit-v1.md §E`.
This is the acceptance surface: the library behavior a fix should change so the
documented CLI/skill path reaches the same closure without a hand-written
harness.

**Defect 1 (High) — the documented CLI emits no closure.** The skill and
`release-trains/README.md` tell a user to run
`synchronizer release-train <config> <intent>`. That command runs `execute()`
and renders the cascade report, then stops (`src/main.rs`): no `resolve_closure`,
no `release-train.lock.json`, no integration flake, no closure identity. To reach
the closure a user must write Rust — this whole crate exists only because that
Rust does not ship. Every stage c–f here is caller glue the CLI should own.

**Defect 2 (High) — discovery does not connect to `resolve_closure`.**
`DependencyGraph::discover` is callable and produces a correct graph (proved
here: 6 components, 21 edges, valid ascent). But its result type does not
compose with `MaterializedReleaseTrain::resolve_closure`, which requires
`discovered_internal_components: BTreeSet<ComponentName>` and
`discovered_external_components: BTreeMap<ComponentName, CommitIdentifier>`. The
exact gaps:

- `DependencyGraph` exposes no accessor for its component set — the `components`
  field is private and only `edges()`, `dependencies_of()`, and
  `ascent_levels()` are public — so the internal set must be hand-assembled from
  the manifest list the caller passed in.
- `discover()` deliberately drops every edge that points outside the configured
  set (third-party out of scope), so it never records external components with
  their commits. The `discovered_external_components` map cannot be obtained from
  discovery at all; a caller must build it independently.

So discovery runs, but the two membership-validation inputs are hand-built here.
A fix wants `discover()` (or a method on `DependencyGraph`) to yield the exact
`(internal set, external commit map)` `resolve_closure` consumes, and
`ReleaseTrainRun::execute` to wire it in so undeclared-edge / unadmitted-external
failures are reachable from a real run.

**Defect 3 (High) — the generated integration flake does not evaluate under
Nix.** `to_integration_flake` emits inputs as
`{ url = "github:…/rev"; narHash = "sha256-…"; }`. Nix rejects it:

```
error: unexpected flake input attribute 'narHash', at flake.nix:4:5
```

`narHash` is not a valid flake *input* attribute (it lives in `flake.lock`), so
the emitted flake is not merely untested — it is not evaluable. The
`release-train.lock.json` is valid Nix-consumable data (`builtins.fromJSON`
reads it), but the flake wrapper around it is invalid.

*Fix validated in this run:* move the narHash into the input URL as a query
parameter — `url = "github:owner/repo/rev?narHash=sha256-…"`. With that one
change, `nix flake metadata` locks and narHash-verifies all six candidate
inputs (plus their transitive `rust-build`/`fenix`/`nixpkgs` inputs), and
`nix eval path:…#releaseTrain.identity` returns `42df158c…` — the same closure
identity the library computed. So the six candidate commits are genuinely
portable, fetchable, and narHash-verified by Nix; the only blocker is the
invalid input-attribute emission.

**Defect 4 (Medium) — expected-base laundering, unobservable here.** With
all-`Mainline` selectors, each expected base equals the selected `main` tip, so
the ancestor check is trivially satisfied and the live path's
`observed_base = expected_base` makes the equality validator a no-op. A real
drift-rejection cannot be exercised from a genuine run until the run records the
truly observed base; this run could not distinguish drift.

**Defect 5 (Medium) — no component-check orchestration, confirmed at the Nix
level.** Even the *corrected* flake's `outputs` is only
`releaseTrain = builtins.fromJSON …`. Nix proves the inputs are fetchable and
narHash-verified; it does **not** build the six components or run their checks at
the candidate commits, and there is no closure-identity-plus-check co-report. A
green eval here is not a green build of the train.

**Defect 6 (Medium) — docs overstate current behavior.**
`release-trains/README.md` lists `release-train.lock.json` and the integration
flake as outputs of the CLI command. The command produces neither; confirmed by
running the real chain.

**Defect 7 (Low) — the run captures no attestations or lock identities.**
`resolve_closure` requires the caller to supply every `NixSourceAttestation` and
`ComponentLockIdentity`; `execute()` computes none for the closure (its
`nar_hash_source` is used only for flake-lock bumps). All six narHashes and six
lock blake3s in this closure were computed by the harness
(`DogfoodHarness::attest_selectors`), not by the run. A fix wants materialization
to capture real prefetch narHash and per-component lock content into the closure.

**Additional friction — `execute()` is heavier than "push bootstrap
branches".** `execute()` also runs the full StagedCascade, whose per-component
verify resolves a builder host and runs `nix build` over ssh. A user without a
resolvable builder gets a `RoleResolution` failure in the report (as this run
did, deliberately). The closure path and the ssh-build verify are entangled in
one entry point.

## Mechanics

Standard flake per repo convention: `nix flake check` gates `build`, `test`,
`fmt`, `clippy` (`-D warnings`), and `doc`. The pure `tests/intent.rs` check
proves the authored intent decodes to the six mainline members without touching
the network. The stage b–g drive needs the network (git push, `nix flake
prefetch`, flake eval) and runs from the binary, not the sandboxed checks.
