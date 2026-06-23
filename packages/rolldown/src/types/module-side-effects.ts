import type { StringOrRegExp } from './utils';

interface ModuleSideEffectsRule {
  test?: RegExp;
  external?: boolean;
  sideEffects: boolean;
}

interface PureTopLevelCallsRule {
  test?: RegExp;
  external?: boolean;
  pure: boolean;
}

type ModuleSideEffectsOption =
  | boolean
  | readonly string[]
  | ModuleSideEffectsRule[]
  | ((id: string, external: boolean) => boolean | undefined)
  | 'no-external';

type PureTopLevelCallsOption =
  | boolean
  | StringOrRegExp
  | readonly StringOrRegExp[]
  | PureTopLevelCallsRule[]
  | ((id: string, external: boolean) => boolean | undefined);

/**
 * When passing an object, you can fine-tune the tree-shaking behavior.
 */
export type TreeshakingOptions = {
  /**
   * **Values:**
   *
   * - **`true`**: All modules are assumed to have side effects and will be included in the bundle even if none of their exports are used.
   * - **`false`**: No modules have side effects. This enables aggressive tree-shaking, removing any modules whose exports are not used.
   * - **`string[]`**: Array of module IDs that have side effects. Only modules in this list will be preserved if unused; all others can be tree-shaken when their exports are unused.
   * - **`'no-external'`**: Assumes no external modules have side effects while preserving the default behavior for local modules.
   * - **`ModuleSideEffectsRule[]`**: Array of rules with `test`, `external`, and `sideEffects` properties for fine-grained control.
   * - **`function`**: Function that receives `(id, external)` and returns whether the module has side effects.
   *
   * **Important:** Setting this to `false` or using an array/string assumes that your modules and their dependencies have no side effects other than their exports. Only use this if you're certain that removing unused modules won't break your application.
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
   * Treat call and `new` expressions executed during module initialization as side-effect-free when
   * they appear in assignment-like contexts for matching modules.
   *
   * This includes variable initializers, assignment right-hand sides for local bindings, exported
   * expression values, and nested argument calls in those contexts. Calls used as the callee of
   * another call or `new` expression, and standalone expression statements such as `fn()`, are not
   * made pure by this option.
   *
   * **Values:**
   *
   * - **`true`**: Enable the behavior for every module that Rolldown scans.
   * - **`false`**: Disable the behavior.
   * - **`string | RegExp | Array<string | RegExp>`**: Enable the behavior for matching module IDs.
   * - **`PureTopLevelCallsRule[]`**: Array of rules with `test`, `external`, and `pure` properties for fine-grained control.
   * - **`function`**: Function that receives `(id, external)` and returns whether this behavior applies.
   *
   * **Important:** This is intentionally aggressive and can remove real side effects. Prefer
   * scoping it to trusted modules with a string/RegExp pattern or rule.
   *
   * > [!NOTE]
   * > **Performance: Prefer `PureTopLevelCallsRule[]` or patterns over functions**
   * >
   * > Rules and string/RegExp patterns are processed in Rust. JavaScript callbacks require runtime
   * > calls between Rust and JavaScript and should be reserved for logic that cannot be expressed
   * > declaratively.
   *
   * @example
   * ```js
   * treeshake: {
   *   pureTopLevelCalls: /\/src\//
   * }
   *
   * treeshake: {
   *   pureTopLevelCalls: [
   *     { test: /\/src\//, pure: true },
   *     { test: /\/src\/unsafe\//, pure: false },
   *   ]
   * }
   * ```
   * @default false
   */
  pureTopLevelCalls?: PureTopLevelCallsOption;
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
