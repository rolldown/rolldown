# WASI Platform Tests

This directory contains TypeScript tests for the WASI platform functionality in rolldown_common.

## Test Files

- `platform-strings.test.ts`: Tests for platform string conversion
- `wasi-platform.test.ts`: Tests for WASI platform detection

## Running Tests

To run these tests, you'll need:

1. Node.js installed
2. pnpm or npm for package management

```bash
# Install dependencies
pnpm install

# Run tests
pnpm test
```

**Note**: If you encounter issues with Vite/Vitest due to special characters in the path (like `#`),
you may need to copy or clone the repository to a path without special characters.
