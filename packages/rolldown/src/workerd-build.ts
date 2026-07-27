// High-level rollup-style build API for `@rolldown/browser/workerd`.
//
// This module reuses the existing normalization pipeline (`rolldown()` →
// `createBundlerOptions` → bindingify → `RolldownBuild`) instead of
// reimplementing it. Inside workerd bundles, `src/binding.cjs` is aliased to
// `src/binding-workerd-proxy.ts` (see `build.ts#bundleManagedWorkerdLoaders`),
// so every binding call the pipeline makes is forwarded to the managed
// per-instance exports entered here.
//
// See internal-docs/async-runtime/implementation.md.
import * as bindingNamespace from './binding.cjs';
import { rolldown } from './api/rolldown';
import type { RolldownBuild } from './api/rolldown/rolldown-build';
import { __enterWorkerdBinding, __exitWorkerdBinding } from './binding-workerd-proxy';
import type { InputOptions } from './options/input-options';
import type { OutputOptions } from './options/output-options';
import {
  createInstance,
  type DeferredRolldownInstance,
} from './rolldown-binding.wasip1-deferred.js';
import type { RolldownOutput } from './types/rolldown-output';
import {
  configureAsyncContext,
  getAsyncContextSupport,
  type AsyncContextStorage,
} from './utils/async-context';

export interface WorkerdBuildOptions extends InputOptions {
  /**
   * A live workerd Rolldown instance created by `createInstance()`. The
   * instance stays usable after the build; the caller keeps ownership and
   * disposes it. Pass exactly one of `instance` or `module`.
   */
  instance?: DeferredRolldownInstance;
  /**
   * A precompiled threadless Rolldown Wasm module (for workerd: imported under
   * a `CompiledWasm` rule, e.g. `import mod from '@rolldown/browser/workerd/wasm'`).
   * `build()` creates a private instance for this build and disposes it before
   * returning. Pass exactly one of `instance` or `module`.
   */
  module?: WebAssembly.Module | PromiseLike<WebAssembly.Module>;
  /** Output options for the in-memory `generate()` pass. */
  output?: OutputOptions;
  /**
   * Strip ANSI escape sequences from build error messages and code frames.
   * Defaults to `true`: workerd log sinks are rarely ANSI-capable terminals.
   */
  stripAnsi?: boolean;
}

export interface WorkerdBundleOptions {
  /** See {@linkcode WorkerdBuildOptions.stripAnsi}. Defaults to `true`. */
  stripAnsi?: boolean;
}

/**
 * A rollup-style bundle bound to one workerd Rolldown instance. While the
 * bundle is open (until `close()` settles), the owning instance is the active
 * binding for this module; other instances cannot start builds.
 */
export interface WorkerdBundle {
  /** Whether `close()` has completed (or the underlying build was closed). */
  readonly closed: boolean;
  /** Generate the bundle in memory. May be called multiple times. */
  generate(outputOptions?: OutputOptions): Promise<RolldownOutput>;
  /** Run closeBundle hooks, free native build state, and release the instance. */
  close(): Promise<void>;
}

// Matches the ansi-regex pattern (MIT) used across the ecosystem.
const ANSI_PATTERN = new RegExp(
  [
    '[\\u001B\\u009B][[\\]()#;?]*(?:(?:(?:(?:;[-a-zA-Z\\d/#&.:=?%@~_]+)*',
    '|[a-zA-Z\\d]+(?:;[-a-zA-Z\\d/#&.:=?%@~_]*)*)?(?:\\u0007|\\u001B\\u005C|\\u009C))',
    '|(?:(?:\\d{1,4}(?:;\\d{0,4})*)?[\\dA-PR-TZcf-nq-uy=><~]))',
  ].join(''),
  'g',
);

function stripAnsiText(value: string): string {
  // `String.prototype.replace` with a global pattern always scans from the
  // start and leaves `lastIndex` at 0, so the shared regex stays stateless.
  return value.replace(ANSI_PATTERN, '');
}

function stripAnsiStringProperty(target: object, key: string): void {
  try {
    const value = Reflect.get(target, key);
    if (typeof value !== 'string') return;
    const stripped = stripAnsiText(value);
    if (stripped !== value) {
      Reflect.set(target, key, stripped);
    }
  } catch {
    // Never let error post-processing replace the build error itself.
  }
}

function stripAnsiFromError<T>(error: T): T {
  if (error === null || typeof error !== 'object') return error;
  stripAnsiStringProperty(error, 'message');
  // V8 materializes `stack` at construction with the original colored message,
  // so it must be stripped separately from `message`.
  stripAnsiStringProperty(error, 'stack');
  try {
    const nested = Reflect.get(error, 'errors');
    if (Array.isArray(nested)) {
      for (const item of nested) {
        if (item === null || typeof item !== 'object') continue;
        stripAnsiStringProperty(item, 'message');
        stripAnsiStringProperty(item, 'frame');
        stripAnsiStringProperty(item, 'stack');
      }
    }
  } catch {
    // `errors` is a getter on BundleError; reading it must not break throwing.
  }
  return error;
}

function assertWorkerdBundleContext(): void {
  if (
    (bindingNamespace as { __isWorkerdBindingProxy?: unknown }).__isWorkerdBindingProxy !== true
  ) {
    throw new Error(
      'The workerd build() API is only functional from the bundled @rolldown/browser/workerd ' +
        'entry, where the binding is routed to managed instances. In Node.js or the browser, ' +
        'use the rolldown / @rolldown/browser package APIs instead.',
    );
  }
}

let asyncContextConfigured = false;

// Workerd (with the `nodejs_als` or `nodejs_compat` compatibility flag) and
// Node.js both provide `node:async_hooks`. The workerd bundles are browser
// builds, so the pipeline's required async context has no default provider;
// configure AsyncLocalStorage when the host offers it. Hosts without it keep
// working for builds that never invoke JavaScript callbacks.
async function ensureAsyncContext(): Promise<void> {
  if (asyncContextConfigured || !import.meta.browserBuild) return;
  asyncContextConfigured = true;
  try {
    if (getAsyncContextSupport().source !== 'unavailable') return;
    // Keep the specifier dynamic so bundlers (wrangler/esbuild) do not try to
    // resolve a node builtin statically in workers without nodejs_compat.
    const specifier = 'node:async_hooks';
    const hooks: { AsyncLocalStorage?: new () => unknown } = await import(
      /* @vite-ignore */ specifier
    );
    const AsyncLocalStorageImpl = hooks?.AsyncLocalStorage;
    if (typeof AsyncLocalStorageImpl !== 'function') return;
    configureAsyncContext({
      createStorage: () => new AsyncLocalStorageImpl() as AsyncContextStorage,
    });
  } catch {
    // Leave the async context unavailable; builds that require it report
    // AsyncContextUnavailableError with configuration instructions.
  }
}

function requireInstanceExports(instance: DeferredRolldownInstance): object {
  if (
    instance === null ||
    typeof instance !== 'object' ||
    typeof (instance as { dispose?: unknown }).dispose !== 'function'
  ) {
    throw new TypeError(
      'Expected a workerd Rolldown instance created by createInstance(); ' +
        'pass it as the `instance` option or pass a compiled Wasm `module` instead',
    );
  }
  // Reading `exports` also rejects disposed/disposing instances.
  const exports = instance.exports;
  if (exports === null || typeof exports !== 'object') {
    throw new TypeError('The workerd Rolldown instance did not expose a binding exports object');
  }
  return exports;
}

/**
 * Bundle once with the rollup-style options surface (`input`, `plugins`,
 * `output`, ...) against a managed workerd Rolldown instance, entirely in
 * memory.
 *
 * ```js
 * import { build } from '@rolldown/browser/workerd';
 * import wasmModule from '@rolldown/browser/workerd/wasm';
 *
 * const { output } = await build({
 *   module: wasmModule,
 *   input: 'virtual:entry',
 *   plugins: [myVirtualFilesPlugin],
 *   output: { format: 'esm' },
 * });
 * ```
 */
export async function build(options: WorkerdBuildOptions): Promise<RolldownOutput> {
  const { instance, module, output = {}, stripAnsi = true, ...inputOptions } = options ?? {};
  if ((instance === undefined) === (module === undefined)) {
    throw new TypeError('Pass exactly one of `instance` or `module` to build()');
  }
  assertWorkerdBundleContext();
  await ensureAsyncContext();

  if (module !== undefined) {
    const ownedInstance = await createInstance(module);
    let result: RolldownOutput;
    try {
      result = await buildWithInstance(ownedInstance, inputOptions, output, stripAnsi);
    } catch (error) {
      try {
        ownedInstance.dispose();
      } catch (disposeError) {
        throw new AggregateError(
          [error, disposeError],
          'Build and workerd instance disposal both failed',
          { cause: error },
        );
      }
      throw error;
    }
    ownedInstance.dispose();
    return result;
  }

  return buildWithInstance(instance!, inputOptions, output, stripAnsi);
}

async function buildWithInstance(
  instance: DeferredRolldownInstance,
  inputOptions: InputOptions,
  outputOptions: OutputOptions,
  stripAnsi: boolean,
): Promise<RolldownOutput> {
  const exports = requireInstanceExports(instance);
  __enterWorkerdBinding(exports);
  try {
    const bundle = await rolldown(inputOptions);
    let result: RolldownOutput;
    try {
      result = await bundle.generate(outputOptions);
      // Materialize every lazy output field while this instance's binding is
      // active. The threadless flavor copies output payloads to JavaScript
      // eagerly and frees the native side, so the result stays fully readable
      // after the instance is released or disposed.
      void result.output;
    } catch (error) {
      try {
        await bundle.close();
      } catch (closeError) {
        throw new AggregateError([error, closeError], 'Build and bundle close both failed', {
          cause: error,
        });
      }
      throw error;
    }
    await bundle.close();
    return result;
  } catch (error) {
    throw stripAnsi ? stripAnsiFromError(error) : error;
  } finally {
    __exitWorkerdBinding(exports);
  }
}

/**
 * Create a reusable rollup-style bundle on a caller-owned workerd Rolldown
 * instance. The instance stays the active binding until `close()`; call
 * `generate()` any number of times (including concurrently) in between.
 */
export async function createWorkerdBundle(
  instance: DeferredRolldownInstance,
  inputOptions: InputOptions,
  options: WorkerdBundleOptions = {},
): Promise<WorkerdBundle> {
  const { stripAnsi = true } = options;
  assertWorkerdBundleContext();
  const exports = requireInstanceExports(instance);
  await ensureAsyncContext();

  __enterWorkerdBinding(exports);
  let bundle: RolldownBuild;
  try {
    bundle = await rolldown(inputOptions);
  } catch (error) {
    __exitWorkerdBinding(exports);
    throw stripAnsi ? stripAnsiFromError(error) : error;
  }

  let released = false;
  const release = () => {
    if (released) return;
    released = true;
    __exitWorkerdBinding(exports);
  };

  return {
    get closed(): boolean {
      if (released) return true;
      try {
        return bundle.closed;
      } catch {
        // The instance was disposed out from under the bundle.
        return true;
      }
    },
    async generate(outputOptions: OutputOptions = {}): Promise<RolldownOutput> {
      try {
        const result = await bundle.generate(outputOptions);
        void result.output;
        return result;
      } catch (error) {
        throw stripAnsi ? stripAnsiFromError(error) : error;
      }
    },
    async close(): Promise<void> {
      try {
        await bundle.close();
      } catch (error) {
        throw stripAnsi ? stripAnsiFromError(error) : error;
      } finally {
        release();
      }
    },
  };
}
