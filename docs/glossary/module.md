# Module

A **module** is a single source file (or virtual unit) that Rolldown includes in the module graph — for example a `.js`, `.ts`, or `.jsx` file, or a virtual module provided by a plugin.

Rolldown builds a dependency graph by starting from [entry](./entry.md) modules and following `import` / `require` / dynamic `import()` edges. Each module may be transformed (TypeScript, JSX, plugins) before it is placed into one or more [chunks](./chunk.md).

Related concepts:

- [Barrel Module](./barrel-module.md) — a module that re-exports many other modules
- [Entry](./entry.md) — a module used as a graph starting point
