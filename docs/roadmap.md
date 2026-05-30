# DevImg Roadmap

This roadmap starts after `0.2.7`. The completed v0.2 AI plan has shipped; future work should be smaller, evidence-driven, and compatible with DevImg's local-first contract.

DevImg's durable product constraints remain:

- deterministic image commands stay offline and provider-free;
- provider-backed AI commands stay explicit, opt-in, and review-only;
- image bytes are sent to providers only when a user passes `--include-images`;
- DevImg does not store API keys in config, reports, generated artifacts, logs, or package contents;
- DevImg does not publish, commit, post, or rewrite application prose automatically;
- networked remote-image checks must be explicit and read-only unless a later command documents stronger opt-in behavior;
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

## `0.2.11`: Remote Social Image Audit

Goal: make deployed social-preview images observable without downloading remote bytes into the repo or publishing anything.

Scope:

- Add `devimg remote audit` as an explicit networked, read-only command.
- Accept repeatable `--url <page-url>`, `--urls <path>`, and optional `--sitemap <url>` inputs.
- Parse page metadata for Open Graph and X/Twitter image fields: image URL, secure URL, declared width/height, card type, and alt text.
- Validate discovered image URLs with bounded `HEAD` or small `GET` requests: status, redirects, content type, content length, cache headers, ETag, Last-Modified, and dimensions when safely available.
- Emit deterministic JSON by default and optional Markdown summaries for humans and coding agents.
- Report social-preview risks such as missing images, relative URLs, inaccessible images, non-image content types, missing dimensions, oversized bytes, missing alt text, likely aspect-ratio mismatch, weak cache headers, and non-HTTPS image URLs.
- Dogfood against `cleisson.com` homepage, project, blog, resume, and representative content pages after deploy.

Dogfood and testing strategy:

- Use deterministic fake HTTP servers in DevImg tests for normal pages, missing metadata, redirects, image `404`, wrong content types, oversized images, weak cache headers, blocked private URLs, and platforms that reject `HEAD`.
- Keep normal DevImg CI free of real public-network dependencies.
- Use `cleisson.com` as the first real dogfood target because its generated SEO images are owned, public, stable, and already managed by DevImg.
- Run real public URL audits locally first, then consider a manual `workflow_dispatch` job after dogfood proves stable.
- Avoid required tests against arbitrary social-media CDN images because signed URLs, bot protection, rate limits, and header behavior are unstable.
- Use owned public bucket/CDN URLs for later bucket/CDN dogfood instead of third-party assets.

Expected `cleisson.com` smoke:

```bash
devimg remote audit \
  --url https://www.cleisson.com/en-US \
  --url https://www.cleisson.com/en-US/projects/devimg \
  --url https://www.cleisson.com/en-US/blog \
  --output /tmp/cleisson-remote-audit.json \
  --markdown /tmp/cleisson-remote-audit.md
```

AI stance:

- Keep `0.2.11` deterministic and non-AI.
- Allow existing text-only draft flows to summarize a remote audit artifact manually, for example `devimg draft --draft-type social-post-outline --compare-json /tmp/cleisson-remote-audit.json`.
- Defer any future `remote review --ai` command until deterministic remote auditing is stable, secret-free, and byte-free by default.

Safety defaults:

- Only fetch `https://` URLs by default.
- Block localhost, private IPs, link-local IPs, file URLs, and data URLs.
- Use no cookies, auth headers, provider keys, or local `.env` values.
- Limit redirects, response bytes, request timeout, and concurrency.
- Do not persist downloaded image bytes in JSON, Markdown, reports, or package contents.

Reference constraints:

- Next.js remote images require explicit dimensions and strict `remotePatterns`: <https://nextjs.org/docs/pages/api-reference/components/image>.
- Open Graph image metadata includes image URL, secure URL, width, height, and alt fields: <https://ogp.me/>.
- X summary-large cards define image URL, dimensions, byte, format, and alt-text expectations: <https://developer.x.com/cards/types/summary-large-image>.

Non-goals:

- No local mirroring, image transformation, upload, cache invalidation, or social-platform API posting.
- No provider-backed AI review of remote images.
- No default CI network dependency; CI usage should be opt-in/manual until dogfood proves it is stable.

Done criteria:

- `devimg remote audit` can validate public social-preview images for `cleisson.com` without writing repo-tracked files.
- Reports are stable, secret-free, and byte-free.
- Local deterministic commands remain offline unless `remote audit` is explicitly invoked.

## `0.2.12`: Remote Source Snapshot

Goal: make selected remote images reproducible enough for DevImg-managed local transforms.

Scope:

- Add an opt-in remote source snapshot workflow that downloads allowlisted remote images into a project cache directory and records a lockfile.
- Lock URL, resolved URL, SHA-256, bytes, dimensions, content type, ETag, Last-Modified, fetched-at policy marker, and cache-control metadata.
- Require explicit allowlists for hostnames and path patterns.
- Reuse existing local transform planning only after a remote image has been snapshotted into a local cache path.
- Add a check mode that fails when a locked remote image changes unexpectedly or becomes unavailable.

Non-goals:

- No background refresh.
- No credentials in config.
- No direct transforms from arbitrary remote URLs.

Done criteria:

- Remote snapshots are reproducible, reviewable, and safe to commit when the project chooses to commit them.
- Lockfile changes clearly distinguish URL metadata changes from image-byte changes.

## `0.2.13`: Bucket And CDN Audit

Goal: help teams validate bucket/CDN-hosted image URLs before adding any publish workflow.

Scope:

- Add read-only audit support for S3/R2/GCS/CDN-style public image URL sets.
- Check status, content type, cache-control, ETag, Last-Modified, content length, immutable-hash alignment, and byte budgets.
- Compare audited remote URLs against DevImg manifest/export paths when a local manifest is provided.
- Report stale URLs, weak cache headers for hashed filenames, mutable headers for non-hashed filenames, missing objects, and content-type mismatches.

Non-goals:

- No bucket listing APIs.
- No signed URL generation.
- No upload, delete, purge, or cache invalidation.

Done criteria:

- Teams can validate public bucket/CDN image behavior from a URL list or manifest export without credentials.
- Findings are safe to run in CI when external network access is acceptable.

## `0.2.14`: Remote Publish Design Gate

Goal: decide whether DevImg should ever upload or sync generated assets, and define the safety model before implementation.

Scope:

- Design, but do not yet implement, provider-specific publish/sync contracts for S3, Cloudflare R2, and generic object stores.
- Define dry-run diff output, credential discovery rules, overwrite/delete protections, cache-control policy, content-type mapping, checksum verification, and rollback expectations.
- Decide whether publishing belongs in DevImg core, the CLI only, or external project scripts that consume manifest exports.

Non-goals:

- No production upload implementation in this release.
- No destructive object-store operations.
- No social media posting APIs.

Done criteria:

- A later publish implementation has a decision-complete spec, or publishing is explicitly rejected in favor of external deployment tools.

## Later Candidates

Consider these only after the `0.2.8` through `0.2.14` work is complete or deliberately reprioritized:

- Real Anthropic provider calls for AI review, alt-text drafts, and prose drafts.
- Broader dogfooding for blog images, favicon/app icons, and experience logos in `cleisson.com` when there is measurable churn, size risk, or review value.
- Richer review artifacts for visual comparison, crop focus, and budget trend inspection.
- Optional machine-readable schemas for generated artifacts.
- Social preview cache-refresh integrations, only if they can be explicit, provider-safe, and non-spammy.

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
- explicit networked-command verification for any release that changes remote-image behavior.
