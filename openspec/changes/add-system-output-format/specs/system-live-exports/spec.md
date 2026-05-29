## ADDED Requirements

### Requirement: Variable initializer export

When an exported variable is declared with an initializer, the initializer SHALL
be wrapped in an `exports()` call so the SystemJS runtime is notified at
declaration time.

#### Scenario: let declaration with initializer

- **WHEN** module contains `export let x = 10;`
- **THEN** generated code contains `let x = exports('x', 10);`

#### Scenario: var declaration with initializer

- **WHEN** module contains `export var count = 0;`
- **THEN** generated code contains `var count = exports('count', 0);`

#### Scenario: Uninitialized export

- **WHEN** module contains `export let x;` with no initializer
- **THEN** generated code contains `let x = exports('x', void 0);` (or
  equivalent undefined notification)

### Requirement: Simple assignment export update

Every assignment to an exported mutable binding SHALL be wrapped so `exports()`
is called with the new value at the assignment site.

#### Scenario: Simple reassignment

- **WHEN** exported `let x` is later assigned `x = 5;`
- **THEN** generated code at that statement is `exports('x', x = 5);`

#### Scenario: Assignment inside conditional

- **WHEN** exported variable is assigned inside an `if` body
- **THEN** the assignment is still wrapped: `exports('x', x = value);`

### Requirement: Compound assignment export update

Compound assignment operators (`+=`, `-=`, `*=`, `/=`, `%=`, `**=`, `<<=`,
`>>=`, `>>>=`, `&=`, `|=`, `^=`, `&&=`, `||=`, `??=`) to exported bindings SHALL
be wrapped in `exports()`.

#### Scenario: += operator

- **WHEN** exported `let n` is assigned `n += 1;`
- **THEN** generated code is `exports('n', n += 1);`

#### Scenario: ||= operator

- **WHEN** exported `let val` is assigned `val ||= defaultValue;`
- **THEN** generated code is `exports('val', val ||= defaultValue);`

### Requirement: Prefix increment/decrement export update

Prefix `++` and `--` on exported bindings SHALL be wrapped in `exports()`.

#### Scenario: Prefix increment

- **WHEN** exported `let n` is updated with `++n;`
- **THEN** generated code is `exports('n', ++n);`

#### Scenario: Prefix decrement

- **WHEN** exported `let n` is updated with `--n;`
- **THEN** generated code is `exports('n', --n);`

### Requirement: Postfix increment/decrement export update

Postfix `++` and `--` on exported bindings SHALL be wrapped so the exported
value is the post-operation value (consistent with Rollup reference
implementation).

#### Scenario: Postfix increment

- **WHEN** exported `let n` is updated with `n++;`
- **THEN** generated code exports the incremented value (verify exact form
  against `system-export-rendering` fixture)

#### Scenario: Postfix decrement

- **WHEN** exported `let n` is updated with `n--;`
- **THEN** generated code exports the decremented value in a form consistent
  with the Rollup reference fixture

### Requirement: Destructuring assignment export update

When exported bindings are assigned via destructuring, each exported name in the
pattern SHALL trigger an `exports()` call.

#### Scenario: Array destructuring of exported vars

- **WHEN** `export let a, b;` and later `[a, b] = fn();`
- **THEN** generated code calls `exports` for both `a` and `b` at the
  destructuring site

#### Scenario: Object destructuring of exported vars

- **WHEN** `export let x, y;` and later `({ x, y } = obj);`
- **THEN** generated code calls `exports` for both `x` and `y`

### Requirement: Function declaration hoisted export

Exported function declarations SHALL have their export announced in a hoisted
block that appears before the `execute` body runs (taking advantage of JS
function hoisting so the function exists at the time the announcement is made).

#### Scenario: Exported function declaration

- **WHEN** module contains `export function greet() {}`
- **THEN** `exports('greet', greet)` appears in a block prepended before the
  execute body, and the function declaration itself is unchanged inside execute

### Requirement: Class declaration export

Exported class declarations SHALL be emitted inside execute followed immediately
by an `exports()` call.

#### Scenario: Exported class declaration

- **WHEN** module contains `export class Foo {}`
- **THEN** generated code inside execute contains
  `class Foo {} exports('Foo', Foo);`

### Requirement: Const and inlined bindings are NOT live-wrapped

Exported `const` bindings and bindings proven non-reassigned SHALL NOT have
per-assignment `exports()` injection; a single initial `exports()` call at
declaration is sufficient.

#### Scenario: Const export does not use live wrapping

- **WHEN** module contains `export const PI = 3.14;`
- **THEN** generated code contains `const PI = exports('PI', 3.14);` with no
  additional `exports('PI', ...)` elsewhere

### Requirement: Batch export form for multiple simultaneous exports

When multiple exported bindings are initialized or updated simultaneously (e.g.,
at the top of the execute body), the bundler MAY use the object batch form
`exports({ a: a, b: b })` to reduce call overhead.

#### Scenario: Batch export at execute start

- **WHEN** module exports multiple names that can be announced together
- **THEN** generated code MAY use `exports({ name1: value1, name2: value2 })`
  instead of separate calls

### Requirement: Missing export shim

When `output.shimMissingExports` is `true` and a named export is referenced but
not defined, the bundler SHALL emit `exports('missingName', void 0)`.

#### Scenario: Missing export shimmed

- **WHEN** a chunk re-exports a name that doesn't exist in the source and
  `shimMissingExports` is `true`
- **THEN** output contains `exports('missingName', void 0)`
