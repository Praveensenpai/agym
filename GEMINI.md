# Project Rules & Guidelines

## 1. Explicit Approval Protocol (Mandatory)
Never edit any code files or execute destructive modifications without first explaining:
1. **WHY**: The specific rationale and root cause for the proposed change.
2. **WHAT EFFECT or FIX**: The exact behavior, bug fix, or improvement it will produce.
3. **APPROVAL**: Await explicit user confirmation before applying initial file modifications.

### Operational Boundaries:
- **Initial Proposals & Feature Requests (Approval Mandatory)**:
  When asked to implement a new feature, perform a refactor, or fix an issue, never silently edit code. Always explain the root cause/rationale, expected impact, and obtain explicit user approval before starting modifications. If requesting mobile approval via Tayori, strictly pass the interactive flag: `tayori ask -i "<question>"`. Never run fire-and-forget approval requests without `-i`.
- **Autonomous Task Execution & Error Self-Healing (Zero Permission-Seeking)**:
  Once the user approves a task or fix, the agent has full authorization to carry it through to completion. If the agent's changes introduce compiler errors, clippy warnings, test failures, or secondary bugs during implementation, the agent MUST NOT stop and ask for permission to fix them. NEVER ask "I made a mistake / there's another error, should I fix it?"—own the error and resolve it autonomously in a closed loop until the task is complete and verified green.
- **Mandatory Active Verification & Proof Protocol (Zero Assumption)**:
  Never assume code works without active verification. Before declaring any coding task, bug fix, or refactor complete, the agent MUST actively execute the 3-phase verification cycle (`skills/build-tooling/systematic-code-verification/`): static validation/compilation, test suite execution, and live runtime smoke testing. The agent must provide verifiable terminal output proving the fix works. Never say "it should work" or ask the user to test what the agent can test itself.
- **Autonomous Release & CI/CD Self-Healing (Zero Permission-Seeking)**:
  Once a release is initiated or approved, the agent is 100% responsible for delivering a verified green pipeline. If GitHub Actions, compilation, or release workflows fail due to an error introduced during release/build, the agent MUST autonomously diagnose (`gh run view --log-failed`), fix the error, re-test, re-tag/re-push, and monitor until green.

---

## 2. Language & Engineering Standards
All implementations must strictly adhere to the corresponding domain skills in Karakuri:

- **Systematic Code Verification** (`skills/build-tooling/systematic-code-verification/`):
  - **Zero Assumptions**: Code is presumed broken until proven working via active command execution.
  - **3-Phase Verification**: Mandatory static check (compile/lint/typecheck) -> automated tests (unit/integration) -> live runtime smoke test.
  - **Evidence-Based Proof**: Present terminal command output and exit codes before declaring task completion.

- **Rust Projects** (`skills/build-tooling/rust-clean-code/`):
  - **Hard Limits**: <400 lines/file (300 soft), <60 lines/fn (40 soft), max 4 parameters, max 3 nesting depth.
  - **Zero Tolerance**: No `#[allow(dead_code)]`, no `unwrap()`/`expect()` in prod, no old `mod.rs` (use modern `foldername.rs`), 0 compiler/clippy warnings.
  - **Role-Based Architecture**: `domain/`, `infra/`, `api/`.
  - **DRY & Idiomatic**: Centralize shared logic, use traits and guard clauses.

- **Python Projects** (`skills/build-tooling/python-clean-code/`):
  - **Tooling**: Exclusively managed via `uv` (no raw `pip`).
  - **Typing & Formatting**: 100% type annotations, automated `ruff check --fix`, import sorting (`isort`), and `ruff format` on every change.
  - **Zero Tolerance**: No unverified `# type: ignore` or `# noqa`.

- **Jetpack Compose Projects** (`skills/mobile-dev/compose-clean-code/`):
  - **Hard Limits**: <400 lines/file (300 soft), <60 lines/composable (40 soft), max 5 parameters, max 3 nesting depth, max 6 modifier chain.
  - **Zero Tolerance**: Zero emojis in UI/layouts (strictly use Google Material theme icons, defaulting to Material icons if unspecified), no business logic/IO/coroutines in composition, modifier first-optional on root only, no ViewModel passing to children, immutable collections only, no `@Suppress` on Compose lints, stable lazy list keys.
  - **Architecture & Theming**: Two-layer `XRoute` (ViewModel wiring) & `XScreen` (stateless), `collectAsStateWithLifecycle()`, Material 3 semantic tokens, type-safe navigation (`@Serializable` routes).

- **Mobile Notifications & Approvals** (`skills/ai-agents/tayori/`):
  - **Mandatory Interactive Approval**: When requesting human approval via Tayori, strictly run `tayori ask -i "<question>"`. Fire-and-forget `tayori ask` without `-i` is strictly prohibited because it exits immediately without waiting for human confirmation.
  - **Milestone & Error Alerts**: Use `tayori done` upon completing multi-step workflows, and `tayori alert` for critical failures.

- **Releases & Versioning** (`skills/build-tooling/git-release-craft/`):
  - **Mandatory Binary Release Trigger**: For any project producing compiled binaries or compile-time embedded assets (e.g. Rust, Go, C/C++), modifying code, dependencies, or embedded assets automatically mandates the complete release lifecycle (version bump, tag, release notes, publish, CI verification). Never stop at `git push` or leave binary users with stale distributions.
  - **Release Workflow**: Mandatory Linux x86_64 GitHub Actions release workflow (`.github/workflows/release.yml`) for all compiled binary projects (`x86_64-unknown-linux-gnu`).
  - **Release Notes**: Aesthetic highlight format with icons and direct install commands.
  - **Autonomous Workflow Verification**: Actively track GitHub Actions CI/Release runs until green before declaring release complete. Autonomously diagnose and fix any pipeline failures in a closed self-healing loop without asking permission.

- **Repository Management** (`skills/build-tooling/git-repo-craft/`):
  - **Descriptions**: Minimal yet meaningful (<90 chars), zero filler words, no redundant repo name prefix.
  - **Topics & Verification**: Mandatory 4–8 curated kebab-case topics across domain, language, purpose, and ecosystem, verified via `gh repo view`.

- **Documentation & README Craft** (`skills/build-tooling/aesthetic-readme-craft/`):
  - **Visual Identity**: Thematic emoji + title, bold value proposition, cohesive Shields.io badges (flat-square/for-the-badge).
  - **Show, Don't Just Tell**: Mandatory visual diagram (ASCII box flow or Mermaid chart), `🪄 One-Liner Magic` installer, and emoji-categorized feature matrix.

- **Bash & Shell Scripts** (`skills/system-ops/bash-clean-code/`):
  - **Preamble**: Mandatory `set -euo pipefail` and `IFS=$'\n\t'`.
  - **Zero Tolerance**: Zero ShellCheck warnings (`shellcheck -x`), mandatory trap cleanups for tempfiles, XDG compliance, strict variable quoting.

- **AI Codebase Index** (`skills/build-tooling/codebase-digest/`):
  - **Living Semantic Index**: Mandatory AI-first `CODEBASE.md` maintained in the root of the project with zero fluff, dense symbol skeletons, and module dependencies.
  - **Iterative Auto-Update**: At the end of every turn/iteration involving file additions, deletions, renames, or signature modifications, `CODEBASE.md` must be updated before finishing the task.

- **Writing & Prose** (`skills/writing/unslop/`):
  - **AI Tell Removal**: Strip all AI vocabulary (delve, crucial, fostering, pivotal, vibrant, tapestry, testament, underscore, etc.), sycophantic phrases, em-dash overuse, boldface overuse, and filler phrases from every response, comment, commit message, and doc.
  - **Karakuri Emoji Exception**: Emojis are allowed in README headings and feature bullet lists (required by `aesthetic-readme-craft`). Remove emojis from code comments, commit messages, and inline prose only.
  - **Direct Communication**: No "I hope this helps!", "Of course!", "Great question!", or hedging chains. Respond directly.
