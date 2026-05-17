# DevImg v0.2 AI Plan

## Objective

Add optional AI-assisted workflows that help developers review images, generate suggestions, and write project-facing guidance without making image generation non-deterministic.

AI should advise. DevImg's Rust pipeline should continue to execute deterministic transforms, manifests, reports, and CI checks.

## Principles

- Opt-in only: no image or project data leaves the machine unless the user explicitly runs an AI command.
- Privacy first: commands must describe what files may be sent to a model before doing so.
- Suggestions, not silent edits: AI output should be written as reviewable suggestions, not applied automatically.
- Deterministic core: `optimize`, `check`, `manifest export`, `compare`, and `review` must remain deterministic and model-free.
- Agent compatible: outputs should be structured enough for Codex, Claude Code, and CI tools to consume safely.
- Provider boundaries: start with provider-neutral interfaces where practical; document any OpenAI-specific behavior when added.

## Candidate Features

### 1. `devimg suggest`

Analyze source images and config to propose safer image settings.

Potential output:

- `devimg-suggestions.json`;
- optional Markdown summary;
- suggested `fit`, `crop`, `quality`, widths, and formats;
- rationale per source/preset;
- warnings for source images that are too small or compositionally risky.

Rules:

- Do not rewrite `devimg.toml` by default.
- Do not overwrite an existing suggestions file unless `--force` is passed.
- Include enough source/preset context for a human or coding agent to apply changes manually.

### 2. `devimg review --ai`

Extend visual review with AI-generated observations.

Potential observations:

- important text or UI cropped;
- logo has too much padding;
- source is too small for requested widths;
- variant likely loses readability at small sizes;
- social/open-graph crop may hide important content.

Rules:

- The HTML review remains usable without AI.
- AI observations must be clearly labeled as suggestions.
- The command should support machine-readable output for agents.

### 3. `devimg alt`

Generate alt-text drafts for static content images.

Potential output:

- source path;
- short alt-text candidate;
- confidence/review note;
- warning for decorative or text-heavy images.

Rules:

- Do not insert alt text into app code automatically.
- Keep generated text in a reviewable file.
- Avoid sending private images unless the user opts in.

### 4. `devimg agent task`

Generate a high-signal task prompt for AI coding tools.

Potential output:

- current image pipeline status;
- files safe to edit;
- files that must be regenerated rather than hand-edited;
- exact commands to run;
- current warnings/issues;
- expected final response format.

Rules:

- Must not overwrite existing agent instruction files.
- Should work without external AI APIs.
- Can be used as a bridge from deterministic DevImg state to Codex/Claude prompts.

### 5. `devimg draft`

Generate release, documentation, and project-page drafts from deterministic DevImg outputs.

Potential inputs:

- manifest;
- Markdown report;
- compare report;
- review artifact summary;
- changelog section.

Potential output:

- README release note draft;
- short project-page copy;
- technical blog outline.

Rules:

- Keep output as drafts.
- Never publish automatically.

## Privacy And Consent Model

Before any AI command sends images or metadata to an external model, DevImg should show:

- provider name;
- files or thumbnails that may be sent;
- whether filenames/paths are included;
- output file path;
- how to run a local-only or dry-run preview if available.

Potential flags:

- `--ai-provider <provider>`;
- `--model <model>`;
- `--include-images`;
- `--metadata-only`;
- `--dry-run`;
- `--output <path>`;
- `--force`.

Default behavior should be conservative:

- no external call unless an AI command is explicitly used;
- no image bytes sent unless `--include-images` or equivalent is explicit;
- no overwrite without `--force`.

## Data Model Ideas

Use structured outputs that can be diffed and consumed by agents:

```json
{
  "version": 1,
  "generated_at": "unix:...",
  "config_path": "devimg.toml",
  "provider": "openai",
  "model": "...",
  "suggestions": [
    {
      "source_path": "public/projects/devimg.png",
      "preset": "project-banner",
      "severity": "advisory",
      "kind": "crop_risk",
      "message": "Important text may be close to the crop edge.",
      "suggested_config": {
        "fit": "contain"
      }
    }
  ]
}
```

## Non-Goals For v0.2

- Do not make AI required for the image pipeline.
- Do not auto-commit, auto-open PRs, or auto-publish posts.
- Do not silently rewrite source images, generated variants, manifests, or app code.
- Do not build hosted storage, accounts, dashboards, or SaaS features.
- Do not send secrets or personal data intentionally; filenames and paths should be treated as potentially sensitive.

## Implementation Phases

### Phase 1: Local AI-Ready Contracts

- Add `devimg agent task` without external model calls.
- Improve JSON outputs where needed for AI agents.
- Document privacy and generated-file boundaries.

### Phase 2: Suggestion Files

- Add `devimg suggest --metadata-only` using deterministic heuristics first.
- Add stable suggestion JSON schema and tests.
- Let AI providers be an optional future backend, not the first dependency.

### Phase 3: Opt-In Vision Review

- Add provider-backed image review behind explicit flags.
- Keep outputs advisory and reviewable.
- Add redaction/privacy docs and tests for refusal to overwrite existing outputs.

### Phase 4: Documentation Drafts

- Add `devimg draft` after release/report data is stable enough.
- Generate reviewable Markdown drafts only.

## Done Criteria For v0.2

- AI features are documented as optional and advisory.
- Deterministic commands remain model-free.
- Every AI command has dry-run or explicit-consent behavior.
- Outputs are structured, diffable, and safe for AI coding agents.
- Tests cover refusal to overwrite existing suggestion files.
- Docs explain privacy, provider configuration, and non-goals.
