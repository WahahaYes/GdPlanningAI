# YouTube Video: Rust Refactor Showcase — Commit List & Highlights

## Branch Landscape

| Branch | Status | Notes | |--------|--------|-------| | `main` | Frozen at
`dc48fe5` (2026-03-12) | Last pure-GDScript state. v0.2.0 with behaviors system,
debugger, community PRs. This is the "before" snapshot. | | `rust` | HEAD
`a9bda00` (2026-07-26) | 328 commits ahead of main. The full Rust refactor story
lives here. | | `origin/campfire_example` | Stale fork at `f0c2c6e` (2026-05-16)
| Branched off rust, added campfire 3D scenes + debug tree experiments. Never
merged back. Campfire scenes live in `examples/` on rust now. Functioning but
not styled. | | `origin/rust_migration` | Stale fork, fully subsumed by `rust` |
No unique commits beyond what rust already has. Can be deleted. |

**Total diff main -> rust:** 284 files changed, 18,349 insertions, 3,180
deletions.

______________________________________________________________________

## Recommended Showstopper Commits (in chronological order)

### Act 1 — The Foundation (Before Rust)

| Commit | Date | Message | Why It Matters |
|--------|------|---------|----------------| | `6afaaf5` | 2025-07-04 | *Initial
GdPlanningAI framework with 2D example* | The very first commit. Origin story.
Quick flash for contrast. | | `dc48fe5` | 2026-03-12 | *Merge PR #33 —
Refactored precondition creation* | **The "before" snapshot.** Last commit on
main before the Rust branch. Pure GDScript GOAP. Shows the v0.2.0 state:
behaviors, debugger, community contributions. Good for a "here's what we had"
intro. |

### Act 2 — The Rust Leap (March 2026)

| Commit | Date | Message | Why It Matters |
|--------|------|---------|----------------| | `83ce398` | 2026-03-21 | *isolate
planning engine to rust* | **THE BIG BANG.** 68 files changed, +7,569 / -903
lines. Rust planning engine replaces GDScript core. This is the commit that
changes everything. Must-show diff. | | `3429296` | 2026-03-22 | *back in
business with working demos* | First proof the Rust bridge actually works
end-to-end. Demos run again. | | `9f23c4f` | 2026-03-27 | *implement messaging
system so we can thread in rust code* | The GDScript-Rust communication layer.
Shows the architectural thinking. | | `7b7b968` | 2026-04-07 | *planning tests
for rust logic* | First real test coverage for the Rust engine. Shows
engineering discipline, not just hacking. |

### Act 3 — The Rewrite(s) (May 2026)

| Commit | Date | Message | Why It Matters |
|--------|------|---------|----------------| | `8683ded` | 2026-05-03 |
*reimplement backward planner* | The algorithm pivots. Previous approach was not
cutting it — this shows the willingness to rebuild from scratch. | | `70f5a94` |
2026-05-04 | *chain provisions and bindings rather than having planner do
guesswork* | A key architectural insight. The planner stops guessing and starts
chaining. | | `02a0a04` | 2026-05-16 | *breakout planner into modular
components* | 12 files, +1,633 / -1,565 lines. Major restructuring. The monolith
becomes modules. Great visual diff. | | `6947d24` | 2026-05-18 |
*reimplementation status* | 33 files, +1,092 / -2,048 lines. The second big
rewrite. Old code dies, new design emerges. Good for showing iterative
development. | | `a85b865` | 2026-05-23 | *yet another rewrite* | Commit message
speaks for itself. Shows the grind. | | `9e72c76` | 2026-05-23 | *djikstra and
yield on each step* | **THE BREAKTHROUGH.** 9 files, +339 / -172 lines.
Dijkstra-based planning with step yielding. This is the algorithm that makes the
system work. Critical commit. | | `a55ea1f` | 2026-05-23 | *rust suite passing*
| The moment it all clicks. Tests pass. The new algorithm works. Emotional
payoff. |

### Act 4 — The Polish (June-July 2026)

| Commit | Date | Message | Why It Matters |
|--------|------|---------|----------------| | `81121d1` | 2026-06-14 | *hash
fingerprinting* | Performance optimization. Shows the system getting smarter,
not just bigger. | | `6ebca94` | 2026-06-14 | *provision action matching to cull
search space* | The planner learns to prune. Search space optimization. | |
`05ff2c8` | 2026-07-03 | *fix planner regressions* | Honest engineering — things
break, you fix them. | | `8ee573e` | 2026-07-03 | *abstract planning algorithm*
| The algorithm becomes a clean, reusable abstraction. Maturity moment. | |
`24ae15f` | 2026-07-03 | *Document cost cache, extract stale-pending helper,
remove ACTIVE_SEARCH_THREADS, add budget tests* | Cleanup + documentation. Shows
the codebase being tamed. | | `bda3639` | 2026-07-12 | *Refactor
process_simulation into single-responsibility helpers* | 7 files, +1,209 / -216
lines. Massive test addition (568 lines of tests). The system becomes
maintainable. | | `5b7425e` | 2026-07-12 | *Refactor expansion insertion into
PlanBranch::insert_action_at* | 6 files, +749 / -162 lines. More surgical
refactoring with focused tests. | | `7b7a264` | 2026-07-25 | *chore: add
pre-commit hooks with gitleaks, rustfmt, clippy, ruff, gdformat* | Engineering
maturity. Tooling. Not glamorous but shows this is a real project. | | `8472404`
| 2026-07-25 | *write an authoring guide* | Documentation for users. The project
is ready for others. | | `a9bda00` | 2026-07-26 | *rewrite pseudocode* | Current
HEAD. The algorithm is documented and clean. Final state. |

______________________________________________________________________

## Suggested Video Structure

### Quick-Cut Montage (30-60s)

Flash through the commit timeline visually — show the repo evolving from a small
GDScript project to a full Rust hybrid. Git log --graph would look great here.
Use commits `6afaaf5` -> `dc48fe5` -> `83ce398` -> `9e72c76` -> `a9bda00` as
keyframes.

### "The Before" (2-3 min)

- Show `dc48fe5` on main — pure GDScript GOAP
- Demo of the old hunger example running
- Explain what GOAP is and why it works, but what the limitations are

### "The Leap" (3-5 min)

- The `83ce398` diff — 7,500+ lines of Rust appearing
- Show the Rust planner structure (`addons/GdPlanningAI/rust/`)
- Explain why Rust: performance, safety, the planning engine is compute-bound

### "The Grind" (3-5 min)

- The rewrites: `8683ded` -> `6947d24` -> `a85b865`
- Show that the first approach did not work, had to rethink
- Key insight: `70f5a94` (provision chaining) and `9e72c76` (Dijkstra)
- Show the "rust suite passing" moment — tests green

### "The Result" (3-5 min)

- `bda3639` / `5b7425e` — the clean, well-tested codebase that emerged
- Side-by-side: old GDScript engine vs new Rust engine
- Show the authoring guide — ready for users
- Demo: campfire scenes, 3D planning, the full feature set

### "What's Next" (1-2 min)

- Campfire examples need styling (call out `origin/campfire_example` branch
  experiments)
- Roadmap items from README TODOs
- Open source pitch — community contributions welcome

______________________________________________________________________

## Key Visual Moments to Capture

1. **The graph**: `git log --oneline --all --graph --decorate` — shows the
   branch topology beautifully
1. **The big bang diff**: `git show --stat 83ce398` — 68 files, massive
   insertions
1. **The algorithm diagram**: The pseudocode in `notes/` that documents the
   Dijkstra-based approach
1. **Tests passing**: `cargo test` output from `a55ea1f` era
1. **Before/after directory tree**: `addons/GdPlanningAI/rust/src/planner/` —
   the modular Rust structure
1. **The debug tree**: The visual debugger showing plan visualization
1. **Campfire 3D**: The campfire scenes that function but are not yet styled —
   shows the breadth of the system

______________________________________________________________________

## Branch Cleanup Notes

- `origin/rust_migration` can be deleted — fully subsumed by `rust`
- `origin/campfire_example` is stale (forked at `f0c2c6e`, May 16) — campfire
  scenes now live on `rust` branch. The branch has some unique debug tree
  experiment commits (`2aabdc5`, `43f6aef`) that were never merged, but those
  features appear to have been reimplemented differently on `rust`. Consider
  archiving or deleting.
- `main` should stay as-is for now — it represents the last "stable release"
  (v0.2.0) and is the base for the Godot Asset Library.
