## ADDED Requirements

### Requirement: Anonymous System.register wrapper

When `output.name` is not set (or is empty), the bundler SHALL emit an anonymous
`System.register` call as the outermost chunk wrapper, with no name argument.

#### Scenario: Anonymous registration emitted

- **WHEN** `output.format` is `"system"` and `output.name` is not set
- **THEN** chunk output begins with `System.register([`, with no string name
  argument

#### Scenario: Module body is inside execute function

- **WHEN** `output.format` is `"system"`
- **THEN** all module source code appears inside a
  `execute: (function () { ... })` property of the returned object

### Requirement: Named System.register wrapper

When `output.name` is set to a non-empty string, the bundler SHALL emit a named
`System.register` call with that string as the first argument.

#### Scenario: Named registration emitted

- **WHEN** `output.format` is `"system"` and `output.name` is `"my-lib"`
- **THEN** chunk output begins with `System.register('my-lib', [`

### Requirement: Factory function shape

The `System.register` factory argument SHALL be a non-arrow function expression.
Its parameter list SHALL include `exports` only when the chunk has exports, and
SHALL include `module` only when the chunk uses dynamic import or `import.meta`.

#### Scenario: No exports, no dynamic import

- **WHEN** chunk has no exports and no dynamic imports or import.meta usage
- **THEN** factory is `(function () {`

#### Scenario: Exports present, no dynamic import

- **WHEN** chunk has at least one export and no dynamic imports or import.meta
  usage
- **THEN** factory is `(function (exports) {`

#### Scenario: Exports and dynamic import both present

- **WHEN** chunk has exports and uses dynamic import or import.meta
- **THEN** factory is `(function (exports, module) {`

#### Scenario: Dynamic import present but no exports

- **WHEN** chunk has no exports but uses dynamic import or import.meta
- **THEN** factory is `(function (module) {` with no `exports` parameter

### Requirement: Strict mode injection

When `output.strict` is `true` (default), the bundler SHALL emit `'use strict';`
as the first statement inside the factory body.

#### Scenario: Strict mode emitted by default

- **WHEN** `output.format` is `"system"` and `output.strict` is not explicitly
  set
- **THEN** `'use strict';` appears as the first line inside the factory function

#### Scenario: Strict mode suppressed

- **WHEN** `output.format` is `"system"` and `output.strict` is `false`
- **THEN** `'use strict';` does NOT appear in the output

### Requirement: Return object structure

The factory function SHALL return an object with a `setters` array and an
`execute` function. The `execute` property SHALL be a regular (non-arrow)
function expression.

#### Scenario: Return object always present

- **WHEN** `output.format` is `"system"`
- **THEN** factory body contains
  `return { setters: [...], execute: (function () { ... }) };`

### Requirement: Async execute for top-level await

When the entry module (or any module in the chunk) uses top-level await, the
`execute` function SHALL be declared as `async function`.

#### Scenario: Async execute emitted for TLA

- **WHEN** the chunk contains a module with top-level `await`
- **THEN** the execute property is `execute: (async function () {`

### Requirement: Code splitting is supported

SystemJS format SHALL NOT force `codeSplitting` to `false`. Multiple output
chunks are valid and expected for code-split builds.

#### Scenario: Code splitting enabled by default

- **WHEN** `output.format` is `"system"` and `codeSplitting` is not explicitly
  disabled
- **THEN** rolldown produces multiple chunks when dynamic imports are present

#### Scenario: Source phase imports rejected

- **WHEN** source code contains `import source x from 'module'`
- **THEN** rolldown emits a build error indicating source phase imports are not
  supported in SystemJS format

### Requirement: output.name with dot-separated namespace is NOT expanded

Unlike IIFE/UMD, SystemJS named registration SHALL pass `output.name` as a plain
string literal to `System.register`. No global namespace object decomposition is
performed.

#### Scenario: Dot-separated name passed verbatim

- **WHEN** `output.format` is `"system"` and `output.name` is `"my.lib.Bundle"`
- **THEN** chunk begins with `System.register('my.lib.Bundle', [`

### Requirement: Banner, footer, intro, outro hooks applied

Standard addon hooks SHALL be applied in the same order as other formats: banner
before the wrapper, intro inside the factory before module sources, outro after
module sources, footer after the closing wrapper.

#### Scenario: Banner placed before System.register

- **WHEN** `output.banner` returns `"/* banner */"`
- **THEN** the string `/* banner */` appears before `System.register` in the
  output

#### Scenario: Intro placed inside factory before module sources

- **WHEN** `output.intro` returns `"/* intro */"`
- **THEN** `/* intro */` appears inside the factory function, before module
  sources
