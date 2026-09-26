# Chunk

A **chunk** is an output file produced by the bundler. It contains one or more [modules](./module.md) and the runtime code needed to load and execute them.

Rolldown creates:

- [Entry chunks](./entry-chunk.md) — one per [entry](./entry.md), exporting that entry’s public API
- Shared / common chunks — modules used by multiple entries or dynamic imports, split out to avoid duplication
- Dynamic import chunks — loaded on demand when the corresponding `import()` runs

How modules are grouped into chunks depends on entries, dynamic imports, and options such as [`codeSplitting`](/reference/OutputOptions.codeSplitting) / manual chunking. See [Automatic Code Splitting](/in-depth/automatic-code-splitting) and [Manual Code Splitting](/in-depth/manual-code-splitting).
