# How Rollup Handles Manual Chunk Names

This document explains how Rollup processes the manual chunk name returned by the user and converts it into the final chunk file name.

## Overview

When users configure `manualChunks`, they provide a string name (alias) for a chunk. This name goes through several transformations before becoming the final file name.

## 1. User Provides the Manual Chunk Name

The user can provide manual chunk names in two ways:

**Object form:**

```javascript
manualChunks: {
  'vendor': ['react', 'lodash']
}
```

**Function form:**

```javascript
manualChunks(id, api) {
  if (id.includes('node_modules')) {
    return 'vendor';
  }
}
```

In both cases, the returned string (e.g., `'vendor'`) is called the **manual chunk alias**.

## 2. Processing in Bundle.ts

The manual chunk name goes through these processing steps:

### For Object Form (src/Bundle.ts:106-122)

```typescript
private async addManualChunks(
  manualChunks: Record<string, readonly string[]>
): Promise<Map<Module, string>> {
  const manualChunkAliasByEntry = new Map<Module, string>();
  const chunkEntries = await Promise.all(
    Object.entries(manualChunks).map(async ([alias, files]) => ({
      alias,
      entries: await this.graph.moduleLoader.addAdditionalModules(files, true)
    }))
  );
  for (const { alias, entries } of chunkEntries) {
    for (const entry of entries) {
      addModuleToManualChunk(alias, entry, manualChunkAliasByEntry);
    }
  }
  return manualChunkAliasByEntry;
}
```

- Creates a map from Module to alias string
- Stores the alias as-is in `manualChunkAliasByEntry`

### For Function Form (src/Bundle.ts:124-147)

```typescript
private assignManualChunks(getManualChunk: GetManualChunk): Map<Module, string> {
  const manualChunkAliasesWithEntry: [alias: string, module: Module][] = [];
  const manualChunksApi = {
    getModuleIds: () => this.graph.modulesById.keys(),
    getModuleInfo: this.graph.getModuleInfo
  };
  for (const module of this.graph.modulesById.values()) {
    if (module instanceof Module) {
      const manualChunkAlias = getManualChunk(module.id, manualChunksApi);
      if (typeof manualChunkAlias === 'string') {
        manualChunkAliasesWithEntry.push([manualChunkAlias, module]);
      }
    }
  }
  // ... sorts and stores aliases
  return manualChunkAliasByEntry;
}
```

- Calls the user's function for each module
- If it returns a string, stores it in `manualChunkAliasByEntry`

### Chunk Creation (src/Bundle.ts:195-211)

The alias is passed to the Chunk constructor when creating chunks.

## 3. Storage in Chunk (src/Chunk.ts)

The Chunk class stores the manual chunk alias:

```typescript
private readonly manualChunkAlias: string | null
```

At construction time (line 247), it creates a suggested variable name:

```typescript
this.suggestedVariableName = makeLegal(this.generateVariableName());
```

## 4. Converting to Variable Name (src/Chunk.ts:812-825)

```typescript
private generateVariableName(): string {
  if (this.manualChunkAlias) {
    return this.manualChunkAlias;
  }
  const moduleForNaming =
    this.entryModules[0] ||
    this.implicitEntryModules[0] ||
    this.dynamicEntryModules[0] ||
    this.orderedModules[this.orderedModules.length - 1];
  if (moduleForNaming) {
    return getChunkNameFromModule(moduleForNaming);
  }
  return 'chunk';
}
```

The `makeLegal()` function (src/utils/identifierHelpers.ts:17-25) transforms the name for use as a JavaScript variable:

```typescript
export function makeLegal(value: string): string {
  value = value
    .replace(/-(\w)/g, (_, letter) => letter.toUpperCase())
    .replace(illegalCharacters, '_');

  if (needsEscape(value)) value = `_${value}`;

  return value || '_';
}
```

**Transformations:**

- Converts dashes to camelCase: `vendor-lib` → `vendorLib`
- Replaces illegal identifier characters with `_`
- Prepends `_` if the name starts with a digit or is a reserved keyword

## 5. Converting to Chunk Name (src/Chunk.ts:494, 968-979)

```typescript
getChunkName(): string {
  return (this.name ??= this.outputOptions.sanitizeFileName(this.getFallbackChunkName()));
}

private getFallbackChunkName(): string {
  if (this.manualChunkAlias) {
    return this.manualChunkAlias;
  }
  if (this.dynamicName) {
    return this.dynamicName;
  }
  if (this.fileName) {
    return getAliasName(this.fileName);
  }
  return getAliasName(this.orderedModules[this.orderedModules.length - 1].id);
}
```

The `sanitizeFileName()` function (src/utils/sanitizeFileName.ts:6-13) ensures the name is safe for file systems:

```typescript
const INVALID_CHAR_REGEX = /[\u0000-\u001F"#$&*+,:;<=>?[\]^`{|}\u007F]/g;
const DRIVE_LETTER_REGEX = /^[a-z]:/i;

export function sanitizeFileName(name: string): string {
  const match = DRIVE_LETTER_REGEX.exec(name);
  const driveLetter = match ? match[0] : '';

  // A `:` is only allowed as part of a windows drive letter (ex: C:\foo)
  // Otherwise, avoid them because they can refer to NTFS alternate data streams.
  return driveLetter +
    name.slice(driveLetter.length).replace(INVALID_CHAR_REGEX, '_');
}
```

**Transformations:**

- Preserves Windows drive letters (e.g., `C:`)
- Replaces invalid file system characters with `_`: `"#$&*+,:;<=>?[]^{|}` and control characters
- Example: `vendor:lib` → `vendor_lib`

## 6. Converting to File Name (src/Chunk.ts:516-551)

```typescript
getPreliminaryFileName(): PreliminaryFileName {
  // ...
  const [pattern, patternName] =
    preserveModules || this.facadeModule?.isUserDefinedEntryPoint
      ? [entryFileNames, 'output.entryFileNames']
      : [chunkFileNames, 'output.chunkFileNames'];
  fileName = renderNamePattern(
    typeof pattern === 'function' ? pattern(this.getPreRenderedChunkInfo()) : pattern,
    patternName,
    {
      format: () => format,
      hash: size => hashPlaceholder || (hashPlaceholder = this.getPlaceholder(patternName, size)),
      name: () => this.getChunkName()
    }
  );
  // ...
}
```

The `renderNamePattern()` function (src/utils/renderNamePattern.ts:7-38) replaces placeholders:

```typescript
export function renderNamePattern(
  pattern: string,
  patternName: string,
  replacements: { [name: string]: (size?: number) => string },
): string {
  return pattern.replace(
    /\[(\w+)(:\d+)?]/g,
    (_match, type: string, size: `:${string}` | undefined) => {
      if (!replacements.hasOwnProperty(type) || (size && type !== 'hash')) {
        return error(/* validation error */);
      }
      const replacement = replacements[type](
        size && Number.parseInt(size.slice(1)),
      );
      // ... validation
      return replacement;
    },
  );
}
```

**Transformations:**

- Replaces `[name]` with the chunk name from `getChunkName()`
- Replaces `[hash]` with a hash placeholder
- Replaces `[format]` with the output format
- Validates that patterns don't contain absolute/relative paths

**Example:**

If `chunkFileNames = "[name]-[hash].js"` and manual chunk name is `"vendor"`:

- Result: `vendor-abc123.js`

## Complete Flow Example

```
User returns: "vendor-lib"
    ↓
Stored as manualChunkAlias in Chunk
    ↓
generateVariableName() → "vendor-lib"
    ↓
makeLegal() → "vendorLib" (for use as JS variable identifier)
    ↓
getFallbackChunkName() → "vendor-lib"
    ↓
sanitizeFileName() → "vendor-lib" (or "vendor_lib" if contains special chars like "vendor:lib")
    ↓
renderNamePattern("[name].js") → "vendor-lib.js"
    ↓
Final chunk file: "vendor-lib.js"
```

## Key Takeaways

1. **The manual chunk name is used directly** for the file name (after sanitization)
2. **For JavaScript variable names**, it's transformed with `makeLegal()` to create valid identifiers
3. **Invalid file system characters** are replaced with `_` by `sanitizeFileName()`
4. **The final file name** comes from the `chunkFileNames` or `entryFileNames` pattern with `[name]` replaced by the sanitized manual chunk name
5. **Two separate transformations** exist:
   - `makeLegal()` for JavaScript variable identifiers (camelCase conversion)
   - `sanitizeFileName()` for file system safety (invalid char replacement)

## Reference Files

- `src/Bundle.ts` - Manual chunk processing and assignment
- `src/Chunk.ts` - Chunk name generation and file name creation
- `src/utils/identifierHelpers.ts` - JavaScript identifier sanitization
- `src/utils/sanitizeFileName.ts` - File system name sanitization
- `src/utils/renderNamePattern.ts` - Pattern replacement for file names
