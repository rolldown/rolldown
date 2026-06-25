# Contribution Guide

Contributions are always welcome, no matter how large or small! Here we summarize some general guidelines on how you can get involved in the Rolldown project.

## Open development

All development happens directly on [GitHub](https://github.com/rolldown/rolldown). Both core team members and external contributors (via forks) send pull requests which go through the same review process.

Outside of GitHub, we also use a [Discord server](https://chat.rolldown.rs) for real-time discussions.

## AI Usage Policy

When using AI tools (including LLMs like ChatGPT, Claude, Copilot, etc.) to contribute to Rolldown:

- **Please disclose AI usage** to reduce maintainer fatigue
- **Discuss before you open a pull request when the change calls for it** — follow the same rules as [Submitting a pull request](#submitting-a-pull-request) below; if you're unsure which path applies, open an issue first
- **You are responsible** for all AI-generated issues or PRs you submit
- **Low-quality or unreviewed AI content will be closed immediately**
- **Contributors who submit repeated low-quality ("slop") PRs will be banned without prior warning.** Bans may be lifted if you commit to contributing to Rolldown in accordance with this policy. You may request an unban via our [Discord](https://chat.rolldown.rs/).

We encourage the use of AI tools to assist with development, but all contributions must be thoroughly reviewed and tested by the contributor before submission. AI-generated code should be understood, validated, and adapted to meet Rolldown's standards.

## Reporting a bug

Please open a bug report on GitHub only after searching the existing issues and finding no match. Be as descriptive as possible, and include all applicable labels.

The best way to get your bug fixed is to include a minimal reproduction — a public repository with a runnable example, a usable code snippet, or a link to our [REPL](https://repl.rolldown.rs/) for a quick in-browser repro.

## Requesting new functionality

Before requesting new functionality, search the [open issues](https://github.com/rolldown/rolldown/issues) — someone may have requested it already. If not, open an issue with the title prefixed with `[request]`. Be as descriptive as possible, and include all applicable labels.

## Submitting a pull request

We welcome pull requests for bugs, fixes, improvements, and new features. Before you open one, please check which of the two paths below applies to your change: [send it directly](#send-a-pull-request-directly), or [discuss the approach first](#discuss-the-approach-first). Either way, be sure your build passes locally before you submit.

For setting up the project's development environment, see [Project Setup](../development-guide/setup-the-project.md).

:::info

Please read the [Etiquette](https://developer.mozilla.org/en-US/docs/MDN/Community/Open_source_etiquette) chapter before submitting a pull request.

:::

### Send a pull request directly

No prior discussion is needed for changes whose correctness speaks for itself:

- Clear bug fixes where the expected behavior is unambiguous
- Documentation, typo, and comment fixes
- Tests for existing behavior
- Small, self-contained internal cleanups with no user-facing change

If there's a related issue, link it in your pull request.

### Discuss the approach first

For the changes below, please open or comment on an issue and reach agreement with the team **before** you start coding or open a pull request:

- New features and new public APIs
- Changes to existing public APIs or to default behavior
- Fixes for an issue that doesn't yet have an agreed-upon approach in the thread

For these changes, the hard part is usually agreeing on the right direction, not writing the code. Talking it through first means your work goes into something we can merge, instead of stalling while the direction is still being worked out.

If you open a pull request in this category without that agreement, we may close it. **Closing it is not a rejection of your work, or of you as a contributor.** It only means the change needs to go through the discussion process first. If you want to drive it forward, share your thinking on the linked issue or in our [Discord](https://chat.rolldown.rs) — once there's agreement on the direction, the pull request is very welcome.

### Draft pull requests

If your pull request is still a work in progress, please open it as a [draft](https://docs.github.com/en/pull-requests/collaborating-with-pull-requests/proposing-changes-to-your-work-with-pull-requests/changing-the-stage-of-a-pull-request) and only mark it **Ready for review** once you genuinely want the team to review it. Converting a PR to "Ready for review" notifies reviewers and code owners, so please hold off until your changes are complete and your build passes locally. This keeps maintainers' inboxes focused on PRs that actually need attention.

### Branch organization

Submit all pull requests directly to the `main` branch. We only use separate branches for upcoming releases or breaking changes; otherwise, everything targets main.

Code that lands in main must be compatible with the latest stable release. It may contain additional features, but no breaking changes. We should be able to release a new minor version from the tip of main at any time.
