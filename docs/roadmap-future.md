# Future Roadmap

This file tracks follow-up work intentionally left outside the `0.2.7` cleanup scope.

## Deferred AI Provider Work

- Add real Anthropic provider calls for AI review, alt-text drafts, and prose drafts after matching provider contracts, privacy behavior, and test fixtures are designed.
- Add CLI-level OpenAI HTTP failure/schema tests through an explicit injectable test transport or local fake server, without adding hidden production configuration or weakening provider-error sanitization.

## Dogfood And CI Polish

- Consider aligning `suggest --check` with acknowledged warning behavior in projects that intentionally keep reviewed warnings visible.
- Consider allowing `suggest` to accept manifest export context when framework warnings depend on checked-in helper drift verification.
- Consider scoped acknowledgements or smarter framework inspection for Next.js projects that already use generated DevImg assets through `unoptimized` images or metadata-only SEO paths.
- Continue keeping dry-run AI artifacts under `/tmp` or `$RUNNER_TEMP` unless a human explicitly promotes reviewed output.
