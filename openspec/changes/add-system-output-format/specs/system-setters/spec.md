## ADDED Requirements

### Requirement: Deps array and setters array are aligned

The deps string array passed to `System.register` and the `setters` array in the
returned object SHALL have the same length and SHALL correspond index-for-index:
`setters[i]` handles imports from `deps[i]`.

#### Scenario: Single external dependency

- **WHEN** chunk imports from one external module `"lodash"`
- **THEN** deps is `['lodash']` and setters has exactly one entry

#### Scenario: Multiple dependencies in consistent order

- **WHEN** chunk depends on two modules in order A, B
- **THEN** `deps[0]` is A's path, `setters[0]` captures A's imports; `deps[1]`
  is B's path, `setters[1]` captures B's imports

### Requirement: Import bindings hoisted as var declarations

All bindings imported from dependencies SHALL be declared as `var` variables in
the factory scope, before the `return` statement, and SHALL be assigned inside
the corresponding setter.

#### Scenario: Named import captured in var

- **WHEN** chunk imports `{ foo }` from `"dep"`
- **THEN** factory body contains `var foo;` before the return, and the setter
  for `"dep"` contains `foo = module.foo;`

#### Scenario: Namespace import captured in var

- **WHEN** chunk imports `* as ns` from `"dep"`
- **THEN** factory body contains `var ns;` before the return, and the setter
  assigns `ns = module;`

#### Scenario: Default import captured in var

- **WHEN** chunk imports `defaultExport` from `"dep"`
- **THEN** factory body contains `var defaultExport;` before the return, and the
  setter contains `defaultExport = module.default;`

### Requirement: Null setters for side-effect-only imports

When `systemNullSetters` is `true` (the default), a dependency whose imports are
not used in the chunk SHALL have `null` as its setter entry instead of
`function(){}`.

#### Scenario: Null setter emitted by default for unused dep

- **WHEN** chunk has a side-effect-only import (`import './side-effect'`) and
  `systemNullSetters` is not set
- **THEN** the corresponding setter entry is `null`

#### Scenario: Empty function setter when systemNullSetters is false

- **WHEN** chunk has a side-effect-only import and `systemNullSetters` is
  `false`
- **THEN** the corresponding setter entry is `function () {}`

### Requirement: Re-export propagation inside setters

When a chunk re-exports a binding from a dependency, the setter for that
dependency SHALL call `exports(name, module.prop)` immediately upon receiving an
update, so downstream modules observe live re-export changes.

#### Scenario: Simple named re-export

- **WHEN** chunk contains `export { foo } from 'dep'`
- **THEN** the setter for `"dep"` contains `exports('foo', module.foo)`

#### Scenario: Renamed re-export

- **WHEN** chunk contains `export { foo as bar } from 'dep'`
- **THEN** the setter for `"dep"` contains `exports('bar', module.foo)`

#### Scenario: Default re-export

- **WHEN** chunk contains `export { default } from 'dep'`
- **THEN** the setter for `"dep"` contains `exports('default', module.default)`

#### Scenario: Batch re-export uses object form

- **WHEN** chunk re-exports multiple bindings from the same dependency
- **THEN** the setter uses the batch object form:
  `exports({ a: module.a, b: module.b })`

### Requirement: export \* handled via \_starExcludes

When a chunk contains `export * from 'dep'`, the factory SHALL declare a
`_starExcludes` null-prototype object containing all of the chunk's own export
names plus `"default"`. The setter for that dependency SHALL iterate the module
object and call `exports` for all keys not in `_starExcludes`.

#### Scenario: \_starExcludes object declared

- **WHEN** chunk contains `export * from 'dep'`
- **THEN** factory body contains
  `var _starExcludes = { __proto__: null, default: 1, ... }` with all own
  exports listed

#### Scenario: Setter iterates non-excluded keys

- **WHEN** chunk contains `export * from 'dep'`
- **THEN** setter for that dep contains
  `for (var name in module) { if (!_starExcludes[name]) setter[name] = module[name]; } exports(setter);`

#### Scenario: Own exports shadow star exports

- **WHEN** chunk has `export const foo = 1` and `export * from 'dep'` where dep
  also exports `foo`
- **THEN** `_starExcludes` contains `foo: 1`, preventing dep's `foo` from
  overriding the local one

### Requirement: Internal chunk dependencies in deps array

When code splitting produces multiple chunks, each chunk SHALL list its static
dependencies on other internal chunks in the deps array, with corresponding
setters.

#### Scenario: Internal chunk dep listed in deps

- **WHEN** chunk A statically imports from chunk B (an internal split chunk)
- **THEN** chunk A's deps array contains B's relative path, and setters has an
  entry capturing the imported bindings

#### Scenario: Dynamic-only dep not in static deps

- **WHEN** chunk only references another chunk via dynamic import (`import()`)
- **THEN** that chunk does NOT appear in the static deps array (it is loaded via
  `module.import()` at runtime)
