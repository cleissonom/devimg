# DevImg Roadmap

This roadmap starts after `0.2.7`. The completed v0.2 AI plan has shipped; future work should be smaller, evidence-driven, and compatible with DevImg's local-first contract.

DevImg's durable product constraints remain:

- deterministic image commands stay offline and provider-free;
- provider-backed AI commands stay explicit, opt-in, and review-only;
- image bytes are sent to providers only when a user passes `--include-images`;
- DevImg does not store API keys in config, reports, generated artifacts, logs, or package contents;
- DevImg does not publish, commit, post, or rewrite application prose automatically;
- generated variants, manifests, reports, review artifacts, and helper exports are regenerated through DevImg commands rather than hand-edited.

## `0.2.8`: Dogfood Diagnostic Precision

Goal: reduce noisy advisory output found during `cleisson.com` dogfooding without weakening safety checks.

Scope:

- Make framework diagnostics more precise for Next.js projects that already render DevImg-owned images with `unoptimized` or use generated assets only in metadata.
- Align `devimg suggest --check` with acknowledged warning behavior so reviewed, scoped warnings do not keep producing blocking warning-level suggestions.
- Allow `suggest` to receive the same manifest export context used by `doctor --export-output`, so helper-drift suggestions can distinguish checked helpers from unknown helpers.
- Add focused tests for acknowledged crop warnings, checked helper exports, and Next.js `unoptimized` consumption patterns.
- Update `cleisson.com` dogfood docs and CI guidance only when behavior changes are proven locally.

Non-goals:

- No new AI provider surface.
- No changes to generated image transform semantics.
- No automatic source/config edits from suggestions.

Done criteria:

- `cleisson.com` can run the intended DevImg doctor/suggest checks without false-positive blocking output.
- Existing warning visibility remains intact in reports and `doctor --json`.
- Current DevImg verification passes: fmt, check, clippy, tests, diff check, security checks, and package `.env` exclusion.

## `0.2.9`: Provider Hardening

Goal: make OpenAI-backed commands easier to test and support without making CI depend on real provider calls.

Scope:

- Add CLI-level provider failure tests through an injectable test transport or local fake server.
- Cover OpenAI transport errors, malformed structured output, schema mismatches, refusal/error payloads, and timeout handling.
- Keep provider error sanitization tests explicit: no API keys, data URLs, image bytes, or raw provider responses should persist in artifacts.
- Document an optional manual `workflow_dispatch` pattern for teams that want CI-assisted drafting with `OPENAI_API_KEY`, while keeping default CI examples dry-run only.
- Keep local `.env` loading limited to explicit AI commands.

Non-goals:

- No default GitHub Action secret requirement.
- No real provider calls in normal CI.
- No Anthropic HTTP implementation unless its contract, fixtures, and privacy behavior are ready.

Done criteria:

- Provider-backed command failures are covered by deterministic tests.
- Real OpenAI smoke remains optional and local/manual.
- Docs clearly state that production CI can remain keyless.

## `0.2.10`: Artifact Contract Stabilization

Goal: formalize the contracts that external projects and coding agents consume before expanding the feature surface again.

Scope:

- Document stable fields for manifest exports, doctor JSON, compare JSON, suggestion JSON, AI review JSON, alt-text JSON, draft Markdown, and agent-task Markdown.
- Decide which artifacts need explicit schema/version fields and migration notes.
- Review whether `doctor`, `suggest`, `review`, `alt`, and `draft` should share more context plumbing for manifest helpers, framework diagnostics, changelog excerpts, and optional artifacts.
- Decide Anthropic's status for the next cycle: implement real calls with matching tests and privacy behavior, or keep it explicitly deferred.
- Add migration guidance for teams using the GitHub Action and checked-in helper exports.

Non-goals:

- No breaking artifact changes without migration notes.
- No hosted service, remote storage, or automatic publishing behavior.
- No broad UI layer.

Done criteria:

- Artifact contracts are documented enough for external projects and agents to rely on them.
- Any schema/version additions are backward compatible or explicitly migrated.
- Release docs and Action docs reflect the stabilized contract.

## Later Candidates

Consider these only after the `0.2.8` through `0.2.10` work is complete or deliberately reprioritized:

- Real Anthropic provider calls for AI review, alt-text drafts, and prose drafts.
- Broader dogfooding for blog images, favicon/app icons, and experience logos in `cleisson.com` when there is measurable churn, size risk, or review value.
- Richer review artifacts for visual comparison, crop focus, and budget trend inspection.
- Optional machine-readable schemas for generated artifacts.

## Release Discipline

Each release should include:

- a narrow changelog entry;
- focused tests scaled to behavioral risk;
- `cargo +1.88.0 fmt --all -- --check`;
- `cargo +1.88.0 check --workspace`;
- `cargo +1.88.0 clippy --all-targets --all-features -- -D warnings`;
- `cargo +1.88.0 test --all`;
- `git diff --check`;
- `scripts/security-checks.sh`;
- package-list checks confirming `.env` files are excluded;
- dogfood verification in `cleisson.com` when public Action behavior, framework diagnostics, or image-management guidance changes.
