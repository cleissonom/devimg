# DevImg v0.2 AI Plan

## Objective

DevImg `v0.2` adds opt-in AI-assisted developer workflows while keeping image generation, manifests, reports, reviews, and CI checks deterministic.

AI-assisted commands produce reviewable files and agent-ready context. They do not silently edit application code, source images, generated variants, manifests, or reports.

## Product Decisions

- DevImg supports Codex, Claude Code, and generic Markdown-based coding agents.
- DevImg supports OpenAI and Anthropic as external AI providers.
- OpenAI authentication uses `OPENAI_API_KEY`.
- Anthropic authentication uses `ANTHROPIC_API_KEY`.
- DevImg never stores API keys in config files, generated artifacts, reports, logs, or test fixtures.
- Deterministic commands remain model-free: `optimize`, `check`, `report`, `inspect`, `doctor`, `manifest export`, `compare`, and non-AI `review`.
- External AI calls run only from explicit AI commands or explicit AI flags.
- Image bytes are never sent to a provider unless the user passes `--include-images`.
- Existing generated files are never overwritten unless `--force` is passed.
- AI output is advisory. DevImg writes suggestions, drafts, or review artifacts for humans and coding agents to inspect.
- `devimg.toml` is never rewritten automatically.

## Release Sequence

`v0.2` ships as small patch releases with fixed scope:

1. `0.2.0`: local agent task contracts for Codex, Claude Code, and generic agents.
2. `0.2.1`: deterministic suggestion files from existing diagnostics.
3. `0.2.2`: suggestion ergonomics, report links, and dogfooding polish.
4. `0.2.3`: OpenAI and Anthropic provider configuration, consent, and mocked client boundaries.
5. `0.2.4`: opt-in AI vision review.
6. `0.2.5`: opt-in alt-text drafts.
7. `0.2.6`: opt-in documentation, release, and project-copy drafts.

## Patch Release Plan

### `0.2.0`: Local Agent Task Contract

Goal: generate high-signal task context for coding agents without network access.

Scope:

- Add `devimg agent task`.
- Add `--agent codex|claude-code|generic`, defaulting to `generic`.
- Read `devimg.toml` by default.
- Keep `--config <path>` for custom config files.
- Summarize `devimg doctor` state:
  - checks;
  - issues;
  - warnings;
  - acknowledged warnings;
  - detected framework;
  - manifest helper paths;
  - generated artifact paths;
  - next commands.
- Include file ownership guidance:
  - files safe for agents to edit;
  - files agents must not hand-edit;
  - commands that regenerate generated artifacts.
- Include expected final-response format for the selected agent.
- Output Markdown to stdout by default.
- Support `--output <path>` for writing a task file.
- Refuse to overwrite existing output unless `--force` is passed.
- Avoid external AI APIs.

Tests:

- stdout output works.
- `--output <path>` writes a file.
- existing output files are protected without `--force`.
- `--config <path>` works.
- `--agent codex`, `--agent claude-code`, and `--agent generic` produce distinct agent guidance.
- projects with warnings include actionable warning context.
- no network access is required.

Done criteria:

- `devimg agent task` works offline.
- Existing deterministic commands keep their current behavior.
- Documentation shows complete Codex and Claude Code workflows using generated task context.

### `0.2.1`: Deterministic Suggestions

Goal: convert existing DevImg diagnostics into stable suggestion files without using an AI provider.

Status: implemented for the `0.2.1` release as a required `devimg suggest --metadata-only` local-only workflow.

Scope:

- Add `devimg suggest --metadata-only`.
- Generate `devimg-suggestions.json` by default.
- Support `--output <path>` for custom JSON output.
- Support `--markdown <path>` for a Markdown summary.
- Use existing deterministic signals:
  - low lossy quality;
  - cover-crop risk;
  - skipped upscale;
  - allowed upscale;
  - generated output larger than source;
  - empty source directory;
  - no generated variants;
  - corrupt or unsupported image;
  - missing output;
  - stale output.
- Include this data per suggestion:
  - schema version;
  - source path;
  - source kind;
  - preset;
  - width;
  - format;
  - warning code;
  - severity;
  - rationale;
  - suggested config patch data;
  - commands to inspect or regenerate.
- Refuse to overwrite existing output unless `--force` is passed.
- Avoid rewriting `devimg.toml`.
- Avoid external AI APIs.

Tests:

- suggestions are deterministic across repeated runs.
- JSON schema stays stable.
- Markdown summary is generated when requested.
- overwrite protection works.
- missing config returns an error with the missing path and next command.
- empty projects produce a valid no-suggestions result.
- warning-heavy projects produce stable suggestion ordering.

Done criteria:

- Suggestions are reproducible and diffable.
- Humans and coding agents can use the output without model calls.

### `0.2.2`: Suggestion Ergonomics And Dogfooding

Goal: make deterministic suggestions easy to interpret in local development and CI.

Status: implemented for the `0.2.2` release as a local-only suggestion check and dogfood workflow.

Scope:

- Add `devimg suggest --check`.
- Add `--fail-on-severity advisory|warning|error`.
- Add terminal summaries for generated suggestions.
- Add links from `devimg doctor`, `devimg check`, and Markdown reports to `devimg suggest` when warnings exist.
- Add examples under `examples/dogfood`.
- Dogfood the deterministic suggestion flow on `cleisson.com` during the `0.2.2` release branch before tagging.
- Document when to acknowledge warnings and when to change config.

Tests:

- `suggest --check` exits successfully when no suggestions meet the fail threshold.
- `suggest --check` fails when suggestions meet the fail threshold.
- report links point to existing suggestion commands and output paths.
- examples run with the published CLI.

Done criteria:

- Every suggestion includes severity, rationale, affected file path, and next command.
- CI can use suggestions as advisory output or as a fail gate.

### `0.2.3`: Provider Configuration And Consent

Goal: add OpenAI and Anthropic provider support behind explicit consent without changing deterministic pipeline behavior.

Status: implemented for the `0.2.3` release as provider setup and consent previews only. Real OpenAI/Anthropic HTTP calls remain deferred to later opt-in AI commands.

Scope:

- Add provider identifiers: `openai` and `anthropic`.
- Read OpenAI credentials from `OPENAI_API_KEY`.
- Read Anthropic credentials from `ANTHROPIC_API_KEY`.
- Add `devimg ai consent`.
- Add common AI flags used by AI-capable commands:
  - `--ai-provider openai|anthropic`;
  - `--model <model>`;
  - `--metadata-only`;
  - `--include-images`;
  - `--dry-run`;
  - `--output <path>`;
  - `--force`.
- Add a consent preview for provider-backed commands.
- Show this data before external calls:
  - provider;
  - model;
  - command;
  - config path;
  - project root;
  - files selected;
  - manifest/report paths;
  - generated outputs when the manifest is readable;
  - whether paths are included;
  - whether image bytes are included;
  - output path.
- Keep `--metadata-only` as the default provider mode.
- Require `--include-images` before image bytes leave the machine.
- Add mocked provider clients for tests.
- Keep provider code isolated from transform, manifest, budget, and check logic.
- Document key setup without printing real keys.
- Do not add provider SDKs, HTTP clients, or model defaults in this release.

Tests:

- missing API key returns a clear provider-specific error.
- keys are never written to output files.
- keys are never logged.
- `--dry-run` prints consent preview and performs no external call.
- provider-backed commands refuse image uploads without `--include-images`.
- mocked OpenAI and Anthropic clients produce stable test outputs.
- deterministic commands do not read provider keys.

Done criteria:

- OpenAI and Anthropic are configured through environment variables.
- No command sends data externally by default.
- Provider tests run without network access.

### `0.2.4`: Opt-In AI Vision Review

Goal: generate AI-assisted visual review observations for selected images.

Status: implemented for the `0.2.4` release as OpenAI-only provider-backed review. Anthropic real review calls remain deferred.

Scope:

- Add `devimg review --ai`.
- Require `--ai-provider openai` for real AI review in this release.
- Require `--model <model>`.
- Require `--include-images` for image-byte review.
- Support `--metadata-only` for path, manifest, dimension, size, and warning review without image bytes.
- Generate `devimg-ai-review.json`.
- Support `--markdown <path>` for a human-readable review.
- Support `--dry-run` without API keys or provider calls.
- Load `OPENAI_API_KEY` only for explicit AI commands, including ignored `.env` files.
- Label all AI observations as advisory.
- Include observation category:
  - crop risk;
  - readability risk;
  - excessive padding;
  - low-resolution source;
  - format/quality concern;
  - accessibility note.
- Include affected source, preset, output variant, severity, rationale, and suggested next command.
- Preserve the existing HTML visual review behavior without AI.

Tests:

- mocked OpenAI review works.
- Anthropic review is rejected clearly while consent previews stay available.
- no network is required in CI.
- existing review artifacts still work without AI.
- overwrite protection works.
- provider failure produces a clear error and leaves existing deterministic artifacts untouched.

Done criteria:

- AI review is fully opt-in.
- HTML review remains renderable without AI.
- AI review output is structured, diffable, and safe for coding agents.

### `0.2.5`: Alt-Text Drafts

Goal: generate reviewable alt-text drafts for static content images.

Scope:

- Add `devimg alt`.
- Support `--ai-provider openai|anthropic`.
- Support `--model <model>`.
- Require `--include-images` for image-byte alt-text generation.
- Support `--metadata-only` for placeholder records without image bytes.
- Generate `devimg-alt.json`.
- Support `--markdown <path>`.
- Include source path, candidate alt text, review note, confidence, image category, and warnings.
- Warn for decorative images, text-heavy images, logos, screenshots, and uncertain descriptions.
- Avoid inserting alt text into application code.

Tests:

- mocked OpenAI alt-text generation works.
- mocked Anthropic alt-text generation works.
- output schema is stable.
- overwrite protection works.
- metadata-only mode does not require image upload.

Done criteria:

- Alt text is clearly marked as draft content.
- Humans review all generated alt text before application use.
- No application code is modified.

### `0.2.6`: Draft Helpers

Goal: generate reviewable documentation, release, and project-copy drafts from DevImg artifacts.

Scope:

- Add `devimg draft`.
- Generate Markdown drafts from:
  - manifest;
  - Markdown report;
  - compare report;
  - visual review summary;
  - AI review summary;
  - changelog section.
- Support draft types:
  - release notes;
  - README snippet;
  - project-page copy;
  - blog outline;
  - social post outline.
- Support deterministic template output without provider credentials.
- Support provider-backed prose with `--ai-provider openai|anthropic` and `--model <model>`.
- Refuse to overwrite existing output unless `--force` is passed.
- Avoid publishing or posting anywhere automatically.

Tests:

- deterministic drafts work without API keys.
- mocked OpenAI drafts work.
- mocked Anthropic drafts work.
- output is clearly marked as draft content.
- overwrite protection works.

Done criteria:

- Draft output contains complete Markdown sections for the selected draft type.
- DevImg never publishes, commits, or posts content automatically.

## Privacy And Consent Model

Provider-backed commands use explicit consent.

Consent preview includes:

- provider name;
- model;
- command;
- config path;
- selected source files;
- selected generated files;
- whether filenames and paths are included;
- whether image bytes are included;
- output file path;
- exact command to run as local-only or dry-run.

Default behavior:

- no external call from deterministic commands;
- no external call from `devimg suggest --metadata-only`;
- no image bytes sent without `--include-images`;
- no overwrite without `--force`;
- no API key values printed to stdout, stderr, reports, JSON, Markdown, or logs.

## Structured Output Contract

AI-readable outputs are versioned JSON with stable field names and deterministic item ordering.

Example suggestion/review record:

```json
{
  "version": 1,
  "generated_at": "unix:...",
  "config_path": "devimg.toml",
  "provider": "openai",
  "model": "example-model",
  "mode": "metadata-only",
  "items": [
    {
      "source_path": "public/projects/devimg.png",
      "preset": "project-banner",
      "output_path": "public/generated/devimg-1200.webp",
      "severity": "advisory",
      "kind": "crop_risk",
      "message": "Important content is close to the configured crop edge.",
      "suggested_config": {
        "fit": "contain"
      },
      "next_command": "devimg review --config devimg.toml"
    }
  ]
}
```

## Documentation Scope

Documentation includes:

- Codex workflow using `devimg agent task --agent codex`.
- Claude Code workflow using `devimg agent task --agent claude-code`.
- Generic agent workflow using `devimg agent task --agent generic`.
- OpenAI setup using `OPENAI_API_KEY`.
- Anthropic setup using `ANTHROPIC_API_KEY`.
- Local-only workflows without provider credentials.
- CI workflows that keep AI disabled.
- Privacy and consent examples.
- Output schema examples for suggestions, reviews, alt text, and drafts.

## Non-Goals For v0.2

- Do not make AI required for the image pipeline.
- Do not auto-commit changes.
- Do not auto-open pull requests.
- Do not auto-publish posts.
- Do not silently rewrite source images.
- Do not silently rewrite generated variants.
- Do not silently rewrite manifests.
- Do not silently rewrite application code.
- Do not build hosted storage.
- Do not build accounts.
- Do not build dashboards.
- Do not build SaaS features.
- Do not send secrets or personal data intentionally.

## Done Criteria For v0.2

- Codex and Claude Code workflows are documented and tested through generated task output.
- OpenAI and Anthropic provider paths are documented, mocked in tests, and protected by consent checks.
- Deterministic commands remain model-free.
- AI commands require explicit provider configuration.
- Image-byte AI review requires `--include-images`.
- Every generated output refuses overwrite without `--force`.
- JSON outputs are structured, versioned, diffable, and safe for coding agents.
- Docs explain local-only mode, provider mode, privacy behavior, and non-goals.
