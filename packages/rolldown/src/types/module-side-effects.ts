interface ModuleSideEffectsRuleBase {
  test?: RegExp;
  external?: boolean;
  sideEffects: boolean;
}

type ModuleSideEffectsRule = ModuleSideEffectsRuleBase & ({ test: RegExp } | { external: boolean });

type ModuleSideEffectsOption =
  | boolean
  | readonly string[]
  | ModuleSideEffectsRule[]
  | ((id: string, external: boolean) => boolean | undefined)
  | 'no-external';

/**
 * When passing an object, you can fine-tune the tree-shaking behavior.
 */
export type TreeshakingOptions = {
  /**
   * **Values:**
   *
   * - **`true`**: Uses package `sideEffects` metadata for bundled modules when available and static analysis otherwise. Rolldown generally treats external modules as having side effects.
   * - **`false`**: Marks all modules as [side-effect-free](https://rolldown.rs/in-depth/dead-code-elimination#marking-entire-modules-as-side-effect-free).
   * - **`string[]`**: Array of module IDs to treat as having side effects. All other modules are marked as side-effect-free.
   * - **`'no-external'`**: Marks external modules as side-effect-free and bundled modules as having side effects, overriding package `sideEffects` metadata and static analysis. To mark only external modules as side-effect-free and preserve the default behavior for bundled modules, use `[{ external: true, sideEffects: false }]`.
   * - **`ModuleSideEffectsRule[]`**: Uses the `sideEffects` value of the first matching rule. Each rule must set `test` or `external`. Unmatched modules use the same behavior as `true`.
   * - **`function`**: Receives `(id, external)` and returns whether the module has side effects. Returning `undefined` uses the same behavior as `true`.
   *
   * **Important:** Mark a module as side-effect-free only if your application can skip evaluating it and any dependencies reached only through it.
   *
   * > [!NOTE]
   * > **Performance: Prefer `ModuleSideEffectsRule[]` over functions**
   * >
   * > When possible, use rule-based configuration instead of functions. Rules are processed entirely in Rust, while JavaScript functions require runtime calls between Rust and JavaScript, which can hurt CPU utilization during builds.
   * >
   * > **Functions should be a last resort**: Only use the function signature when your logic cannot be expressed with patterns or simple string matching.
   * >
   * > **Rule advantages**: `ModuleSideEffectsRule[]` provides better performance by avoiding Rust-JavaScript runtime calls, clearer intent, and easier maintenance.
   *
   * @example
   * ```js
   * // Assume no modules have side effects (aggressive tree-shaking)
   * treeshake: {
   *   moduleSideEffects: false
   * }
   *
   * // Only specific modules have side effects (string array)
   * treeshake: {
   *   moduleSideEffects: [
   *     'lodash',
   *     'react-dom',
   *   ]
   * }
   *
   * // Use rules for pattern matching and granular control
   * treeshake: {
   *   moduleSideEffects: [
   *     { test: /^node:/, sideEffects: true },
   *     { test: /\.css$/, sideEffects: true },
   *     { test: /some-package/, sideEffects: false, external: false },
   *   ]
   * }
   *
   * // Custom function to determine side effects
   * treeshake: {
   *   moduleSideEffects: (id, external) => {
   *     if (external) return false; // external modules have no side effects
   *     return id.includes('/side-effects/') || id.endsWith('.css');
   *   }
   * }
   *
   * // Assume no external modules have side effects
   * treeshake: {
   *   moduleSideEffects: 'no-external',
   * }
   * ```
   *
   * **Common Use Cases:**
   * - **CSS files**: `{ test: /\.css$/, sideEffects: true }` - preserve CSS imports
   * - **Polyfills**: Add specific polyfill modules to the array
   * - **Plugins**: Modules that register themselves globally on import
   * - **Library development**: Set to `false` for libraries where unused exports should be removed
   *
   * @default true
   */
  moduleSideEffects?: ModuleSideEffectsOption;
  /**
   * Whether to respect `/*@__PURE__*\/` annotations and other tree-shaking hints in the code.
   *
   * See [related Oxc documentation](https://oxc.rs/docs/guide/usage/minifier/dead-code-elimination#pure-annotations) for more details.
   *
   * @default true
   */
  annotations?: boolean;
  /**
   * Array of function names that should be considered pure (no side effects) even if they can't be automatically detected as pure.
   *
   * See [related Oxc documentation](https://oxc.rs/docs/guide/usage/minifier/dead-code-elimination#define-pure-functions) for more details.
   *
   * @example
   * ```js
   * treeshake: {
   *   manualPureFunctions: ['console.log', 'debug.trace']
   * }
   * ```
   * @default []
   */
  manualPureFunctions?: readonly string[];
  /**
   * Whether to assume that accessing unknown global properties might have side effects.
   *
   * See [related Oxc documentation](https://oxc.rs/docs/guide/usage/minifier/dead-code-elimination#ignoring-global-variable-access-side-effects) for more details.
   *
   * @default true
   */
  unknownGlobalSideEffects?: boolean;
  /**
   * Whether to assume that invalid import statements might have side effects.
   *
   * See [related Oxc documentation](https://oxc.rs/docs/guide/usage/minifier/dead-code-elimination#ignoring-invalid-import-statement-side-effects) for more details.
   *
   * @default false
   */
  invalidImportSideEffects?: boolean;
  /**
   * Whether to enable tree-shaking for CommonJS modules. When `true`, unused exports from CommonJS modules can be eliminated from the bundle, similar to ES modules. When disabled, CommonJS modules will always be included in their entirety.
   *
   * This option allows rolldown to analyze `exports.property` assignments in CommonJS modules and remove unused exports while preserving the module's side effects.
   *
   * @example
   * ```js
   * // source.js (CommonJS)
   * exports.used = 'This will be kept';
   * exports.unused = 'This will be tree-shaken away';
   *
   * // main.js
   * import { used } from './source.js';
   * // With commonjs: true, only the 'used' export is included in the bundle
   * // With commonjs: false, both exports are included
   * ```
   * @default true
   */
  commonjs?: boolean;
  /**
   * Controls whether reading properties from objects is considered to have side effects.
   *
   * Set to `false` for more aggressive tree-shaking behavior.
   *
   * See [related Oxc documentation](https://oxc.rs/docs/guide/usage/minifier/dead-code-elimination#ignoring-property-read-side-effects) for more details.
   *
   * @default 'always'
   */
  propertyReadSideEffects?: false | 'always';
  /**
   * Controls whether writing properties to objects is considered to have side effects.
   *
   * Set to `false` for more aggressive behavior.
   *
   * @default 'always'
   */
  propertyWriteSideEffects?: false | 'always';
};
