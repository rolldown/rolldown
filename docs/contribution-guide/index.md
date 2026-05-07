# Contribution Guide

Contributions are always welcome, no matter how large or small! Here we summarize some general guidelines on how you can get involved in the Rolldown project.

## Open development

All development happens directly on [GitHub](https://github.com/rolldown/rolldown). Both core team members and external contributors (via forks) send pull requests which go through the same review process.

Outside of GitHub, we also use a [Discord server](https://chat.rolldown.rs) for real-time discussions.

## AI Usage Policy

When using AI tools (including LLMs like ChatGPT, Claude, Copilot, etc.) to contribute to Rolldown:

- **Please disclose AI usage** to reduce maintainer fatigue
- **You are responsible** for all AI-generated issues or PRs you submit
- **Low-quality or unreviewed AI content will be closed immediately**
- **Contributors who submit repeated low-quality ("slop") PRs will be banned without prior warning.** Bans may be lifted if you commit to contributing to Rolldown in accordance with this policy. You may request an unban via our [Discord](https://chat.rolldown.rs/).

We encourage the use of AI tools to assist with development, but all contributions must be thoroughly reviewed and tested by the contributor before submission. AI-generated code should be understood, validated, and adapted to meet Rolldown's standards.

## Reporting a bug

Please report bugs to GitHub only after you have previously searched for the issue and found no results. Be sure to be as descriptive as possible and to include all applicable labels.

The best way to get your bug fixed is to provide a reduced test case. Please provide a public repository with a runnable example, or a usable code snippet. In the future, we will also provide a REPL that runs in the browser for easier reproductions.

## Requesting new functionality

Before requesting new functionality, view [open issues](https://github.com/rolldown/rolldown/issues) as your request may already exist. If it does not exist, submit an issue with the title prefixed with `[request]`. Be sure to be as descriptive as possible and to include all applicable labels.

## Submitting a pull request

We accept pull requests for all bugs, fixes, improvements, and new features. Before submitting a pull request, be sure your build passes locally using the development workflow above.

For setting up the project's development environment, see [Project Setup](../development-guide/setup-the-project.md).

:::info

Please read the [Etiquette](https://developer.mozilla.org/en-US/docs/MDN/Community/Open_source_etiquette) chapter before submitting a pull request.

:::

### Branch organization

Submit all pull requests directly to the `main` branch. We only use separate branches for upcoming releases / breaking changes, otherwise, everything points to main.

Code that lands in main must be compatible with the latest stable release. It may contain additional features, but no breaking changes. We should be able to release a new minor version from the tip of main at any time.
