## ADDED Requirements

### Requirement: Dynamic import rewritten to module.import()

In SystemJS format, dynamic `import()` expressions SHALL be rewritten to
`module.import()`. No Promise shim or wrapper helper is needed.

#### Scenario: Simple dynamic import

- **WHEN** source contains `import('./chunk.js')`
- **THEN** generated code contains `module.import('./chunk.js')`

#### Scenario: Dynamic import with path transformation

- **WHEN** a dynamic import resolves to an internal split chunk with a hashed
  filename
- **THEN** generated code contains
  `module.import('./generated-chunk-abc123.js')` with the resolved output path

#### Scenario: Dynamic import with .then() chaining preserved

- **WHEN** source contains `import('./chunk').then(m => m.foo)`
- **THEN** generated code contains `module.import('./chunk').then(m => m.foo)`
  (chain preserved)

### Requirement: import.meta rewritten to module.meta

In SystemJS format, `import.meta` SHALL be rewritten to `module.meta`.

#### Scenario: Bare import.meta access

- **WHEN** source contains `import.meta`
- **THEN** generated code contains `module.meta`

### Requirement: import.meta.url rewritten to module.meta.url

`import.meta.url` SHALL be rewritten to `module.meta.url`.

#### Scenario: import.meta.url access

- **WHEN** source contains `import.meta.url`
- **THEN** generated code contains `module.meta.url`

### Requirement: Emitted asset file URLs rewritten

`import.meta.ROLLUP_FILE_URL_<refId>` references to emitted assets SHALL be
rewritten to `new URL('<relative-path>', module.meta.url).href`.

#### Scenario: Asset file URL reference

- **WHEN** a plugin emits a file and source uses
  `import.meta.ROLLUP_FILE_URL_<id>`
- **THEN** generated code contains
  `new URL('<emitted-asset-path>', module.meta.url).href`

### Requirement: module parameter deconfliction

When user source code contains a local variable named `module` or `exports`,
those variables SHALL be renamed (e.g., `module$1`, `exports$1`) to avoid
conflicting with the SystemJS factory parameters.

#### Scenario: Local variable named module renamed

- **WHEN** source contains `const module = 1;` inside a chunk that also uses
  dynamic import
- **THEN** generated code renames the user variable: `const module$1 = 1;` and
  the factory parameter remains `module`

#### Scenario: Local variable named exports renamed

- **WHEN** source contains `const exports = {};` inside a chunk that has exports
- **THEN** generated code renames the user variable: `const exports$1 = {};` and
  the factory parameter remains `exports`

### Requirement: module parameter omitted when unused

The `module` parameter SHALL only appear in the factory signature when the chunk
actually uses `module.import()` or `module.meta`. Chunks that use neither SHALL
omit the `module` parameter.

#### Scenario: No dynamic import and no import.meta

- **WHEN** chunk has no dynamic imports and no `import.meta` usage
- **THEN** factory signature does not include `module`

#### Scenario: Dynamic import present

- **WHEN** chunk contains at least one `import()` expression
- **THEN** factory signature includes `module`

#### Scenario: import.meta present

- **WHEN** chunk contains at least one `import.meta` access
- **THEN** factory signature includes `module`
