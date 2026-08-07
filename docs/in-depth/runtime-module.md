# Runtime Module

Rolldown injects a synthetic **runtime module** into every build. It provides the small helper functions the bundler emits when wrapping CommonJS, ESM interop, re-exports, and similar patterns (for example `__esm`, `__commonJS`, and `__toESM`).

You do not import this module from your application code. Rolldown includes it automatically when the output needs one or more of its helpers.

## Module identity

The runtime module is a virtual module with the resolved id `\0rolldown/runtime.js`. The Rolldown package exports this constant as [`RUNTIME_MODULE_ID`](/reference/Constant.RUNTIME_MODULE_ID) for plugin authors.

In bundled output it often appears as a dedicated chunk (commonly named `runtime.js` when using manual code splitting). See [Why there's always a `runtime.js` chunk?](/in-depth/manual-code-splitting#why-there-s-always-a-runtime-js-chunk) for how that chunk is loaded.

## Runtime helpers

The runtime module is authored as JavaScript source in `crates/rolldown/src/runtime/` and parsed at build time. Its named exports are the **runtime helpers** Rolldown may reference from generated code.

The helpers Rolldown knows about are:

- `__create`, `__defProp`, `__name`, `__getOwnPropDesc`, `__getOwnPropNames`, `__getProtoOf`, `__hasOwnProp`
- `__esm`, `__esmMin`
- `__commonJS`, `__commonJSMin`
- `__exportAll`, `__copyProps`, `__reExport`
- `__toESM`, `__toCommonJS`
- `__toBinaryNode`, `__toBinary`
- `__require`

Only the helpers actually needed by the current build are kept. With tree-shaking enabled, an unused runtime module is removed entirely after linking.

## Execution order

The runtime module is always scheduled to run **before** any user module. This guarantees helper functions exist before wrapped modules execute, including when code splitting creates circular chunk dependencies.

## For plugin authors

Plugins can observe and modify the runtime module through the [`transform`](/reference/Interface.Plugin#transform) hook. The hook receives the runtime source with `id` set to `\0rolldown/runtime.js`.

### API contract

When transforming the runtime module:

1. **Preserve every export Rolldown might need.** Do not remove or rename existing helper exports. Rolldown resolves helpers by export name after your transform runs; missing symbols produce a build error.
2. **Preserve export types.** Helpers must remain `export var` bindings with the same names so the AST scanner can track them.
3. **Prefer appending code.** If you need to inject runtime behavior, append to the module (as the built-in HMR plugin does) rather than replacing it.
4. **Mark side effects when needed.** If your additions must always run (for example dev-mode setup), return `moduleSideEffects: true` from `transform` so tree-shaking does not drop the runtime module.

If a transform removes or renames helpers, Rolldown reports which plugin modified the runtime module:

```text
Failed to resolve runtime symbol(s) "__esm" after the runtime module was modified by plugin(s): my-plugin.
Please review these plugins to ensure they do not accidentally remove or rename runtime utilities.
```

### Example: detecting the runtime module

```js
import { RUNTIME_MODULE_ID } from 'rolldown';

export default function myPlugin() {
  return {
    name: 'my-plugin',
    transform(code, id) {
      if (id !== RUNTIME_MODULE_ID) {
        return null;
      }

      // Append custom runtime code; do not remove existing exports.
      return {
        code: `${code}\n// custom runtime setup\n`,
        moduleSideEffects: true,
      };
    },
  };
}
```

Filter the `transform` hook to `id: RUNTIME_MODULE_ID` when possible so Rolldown does not call your plugin for every file. See [Hook Filters](/apis/plugin-api/hook-filters).

## Related reading

- [Bundling CJS](/in-depth/bundling-cjs) — how `__commonJS` wrappers appear in output
- [Manual Code Splitting](/in-depth/manual-code-splitting#why-there-s-always-a-runtime-js-chunk) — the dedicated `runtime.js` chunk
- [Dead Code Elimination](/in-depth/dead-code-elimination) — tree-shaking that determines which helpers are kept
