# Contribution Guide

Contributions are always welcome, no matter how large or small! Here we summarize some general guidelines on how you can get involved in the Rolldown project.

## Open development

All development happens directly on [GitHub](https://github.com/rolldown/rolldown). Both core team members and external contributors (via forks) send pull requests which go through the same review process.

Outside of GitHub, we also use a [Discord server](https://chat.rolldown.rs) for real-time discussions.

## Reporting a bug

Please report bugs to GitHub only after you have previously searched for the issue and found no results. Be sure to be as descriptive as possible and to include all applicable labels.

The best way to get your bug fixed is to provide a reduced test case. Please provide a public repository with a runnable example, or a usable code snippet. In the future, we will also provide a REPL that runs in the browser for easier reproductions.

## Requesting new functionality

Before requesting new functionality, view [open issues](https://github.com/rolldown/rolldown/issues) as your request may already exist. If it does not exist, submit an issue with the title prefixed with `[request]`. Be sure to be as descriptive as possible and to include all applicable labels.

## Submitting a pull request

We accept pull requests for all bugs, fixes, improvements, and new features. Before submitting a pull request, be sure your build passes locally using the development workflow above.

For setting up the project's development environment, see [Project Setup](./setup.md).

### Branch organization

Submit all pull requests directly to the `main` branch. We only use separate branches for upcoming releases / breaking changes, otherwise, everything points to main.

Code that lands in main must be compatible with the latest stable release. It may contain additional features, but no breaking changes. We should be able to release a new minor version from the tip of main at any time.


## Debug with Javascript and Rust

1. Open Javascript Debug Termial
2. Execute any program that works with javascript and rust in Javascript Debug Termial
3. Start Attach:Rust  in Tab `Run and Debug`
4. select javascript process.
5. enjoy mixed debug!

I think you'll still be confused after you read this steps.so you can see this video to learn about this process

[bilibili.com: Debug with Javascript and Rust](https://www.bilibili.com/video/BV1Rm421n79f/)