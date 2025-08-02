# AI Agent Guidelines for Rolldown

This document provides guidelines for AI agents working on the Rolldown project.
It aims to help AI assistants understand the project structure, development
workflows, and contribution standards.

## Project Overview

Rolldown is a JavaScript/TypeScript bundler written in Rust, designed to serve as
the future bundler for [Vite](https://vitejs.dev/). It provides Rollup-compatible
APIs and plugin interfaces while being similar to esbuild in scope.

- **Primary Language**: Rust (bundler core)
- **Supporting Languages**: TypeScript/JavaScript (bindings, tests, examples)
- **Build System**: Just, Cargo, pnpm
- **Testing**: Cargo test, Node.js test suites
- **Documentation**: Available at [rolldown.rs](https://rolldown.rs/)

## Project Structure

```
rolldown/
├── crates/           # Rust crates (core bundler logic)
├── packages/         # Node.js packages and bindings
├── examples/         # Usage examples
├── docs/            # Documentation source
├── tasks/           # Automation tasks
├── scripts/         # Build and utility scripts
└── .github/         # GitHub workflows and templates
```

## Development Guidelines for AI Agents

### Code Changes

- **Minimal Changes**: Make the smallest possible changes to achieve the goal
- **Preserve Functionality**: Never delete or modify working code unless
  absolutely necessary
- **Focus on Task**: Ignore unrelated bugs or broken tests
- **Test Changes**: Always validate that changes don't break existing behavior

### Build and Test Workflow

1. Use `just` commands for building: `just build`, `just test-node`
2. Run formatters: `dprint fmt`
3. Run linters: `oxlint` for TypeScript/JavaScript, `cargo clippy` for Rust
4. Test before and after changes to ensure no regressions

`just init` has already been run, all tools (`cargo-insta`, `cargo-deny`, `cargo-shear`, `typos-cli`) are already installed.

Rust and `cargo` components `clippy`, `rust-docs` and `rustfmt` has already been installed, do not install them.

### File Conventions

- **Rust**: Follow `rustfmt` formatting
- **TypeScript/JavaScript**: Use dprint configuration
- **Documentation**: Follow existing markdown patterns
- **Workflows**: Follow GitHub Actions conventions used in `.github/workflows/`

## Contributing Process

1. **Understand the Issue**: Read issue descriptions and comments thoroughly
2. **Explore First**: Examine existing code patterns before making changes
3. **Plan Changes**: Use minimal modifications approach
4. **Test Iteratively**: Build and test frequently during development
5. **Format Code**: Run `dprint fmt` before submitting changes
6. **Document Changes**: Update relevant documentation if needed

## Useful Commands

### Building

```bash
just build              # Build debug version
just build native       # Build native bindings
just build native release # Build release version
```

### Testing

```bash
just test-node          # Run Node.js tests
just test-rust          # Run Rust tests
just test               # Run all tests
```

### Code Quality

```bash
dprint fmt              # Format all files
just lint               # Run all linters
just roll               # Run comprehensive CI checks locally
```

## Common Patterns

### Rust Code

- Use `anyhow` for error handling
- Follow existing module structure in `crates/`
- Add tests alongside implementation
- Use `tracing` for logging

### TypeScript/JavaScript Code

- Use TypeScript for type safety
- Follow existing test patterns in `packages/`
- Use consistent import/export patterns
- Maintain compatibility with Node.js versions specified in `package.json`

### GitHub Workflows

- Use pinned action versions with SHA hashes
- Include permissions declarations
- Use consistent job naming conventions
- Follow existing patterns for caching and artifact handling

## Resources

- **Contributing Guide**:
  [rolldown.rs/contrib-guide/](https://rolldown.rs/contrib-guide/)
- **Documentation**: [rolldown.rs](https://rolldown.rs/)
- **GitHub Repository**:
  [github.com/rolldown/rolldown](https://github.com/rolldown/rolldown)

## Getting Help

When working on Rolldown as an AI agent:

1. **Read Documentation**: Start with the official documentation and this guide
2. **Examine Existing Code**: Look for similar implementations in the codebase
3. **Follow Patterns**: Maintain consistency with existing code styles and patterns
4. **Test Thoroughly**: Ensure changes don't break existing functionality
5. **Ask for Clarification**: If requirements are unclear, ask for more specific guidance

This document helps ensure AI agents can contribute effectively while maintaining the high quality standards of the Rolldown project.
