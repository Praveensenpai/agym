# Project Rules & Guidelines

## 1. Explicit Approval Protocol (Mandatory)
Never edit any code files or execute destructive modifications without first explaining:
1. **WHY**: The specific rationale and root cause for the proposed change.
2. **WHAT EFFECT or FIX**: The exact behavior, bug fix, or improvement it will produce.
3. **APPROVAL**: Await explicit user confirmation before applying initial file modifications.

### Operational Boundaries:
- **Initial Proposals & Feature Requests (Approval Mandatory)**:
  When asked to implement a new feature, perform a refactor, or fix an issue, never silently edit code. Always explain the root cause/rationale, expected impact, and obtain explicit user approval before starting modifications.
- **Autonomous Task Execution & Error Self-Healing (Zero Permission-Seeking)**:
  Once the user approves a task or fix, the agent has full authorization to carry it through to completion. If the agent's changes introduce compiler errors, clippy warnings, test failures, or secondary bugs during implementation, the agent MUST NOT stop and ask for permission to fix them. NEVER ask "I made a mistake / there's another error, should I fix it?"—own the error and resolve it autonomously in a closed loop until the task is complete and verified green.
- **Autonomous Release & CI/CD Self-Healing (Zero Permission-Seeking)**:
  Once a release is initiated or approved, the agent is 100% responsible for delivering a verified green pipeline. If GitHub Actions, compilation, or release workflows fail due to an error introduced during release/build, the agent MUST autonomously diagnose (`gh run view --log-failed`), fix the error, re-test, re-tag/re-push, and monitor until green.

---

## 2. Language & Engineering Standards
All implementations must strictly adhere to the corresponding domain skills in Karakuri:

- **Rust Projects** (`skills/build-tooling/rust-clean-code/`):
  - **Hard Limits**: <400 lines/file (300 soft), <60 lines/fn (40 soft), max 4 parameters, max 3 nesting depth.
  - **Zero Tolerance**: No `#[allow(dead_code)]`, no `unwrap()`/`expect()` in prod, no old `mod.rs` (use modern `foldername.rs`), 0 compiler/clippy warnings.
  - **Role-Based Architecture**: `domain/`, `infra/`, `api/`.
  - **DRY & Idiomatic**: Centralize shared logic, use traits and guard clauses.

- **Python Projects** (`skills/build-tooling/python-clean-code/`):
  - **Tooling**: Exclusively managed via `uv` (no raw `pip`).
  - **Typing & Formatting**: 100% type annotations, automated `ruff check --fix`, import sorting (`isort`), and `ruff format` on every change.
  - **Zero Tolerance**: No unverified `# type: ignore` or `# noqa`.

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
