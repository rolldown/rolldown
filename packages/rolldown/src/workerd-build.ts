// Inside workerd bundles `src/binding.cjs` is aliased to
// `src/binding-workerd-proxy.ts` (see `aliasWorkerdPipelineModules` in
// `build.ts`), so every binding call the pipeline makes goes to the instance
// entered here.
import * as bindingNamespace from './binding.cjs';
import { build as rolldownBuild } from './api/build';
import type { InputOptions } from './options/input-options';
import type { OutputOptions } from './options/output-options';
import type { RolldownOutput } from './types/rolldown-output';
import { RolldownOutputImpl } from './types/rolldown-output-impl';
import {
  createInstance,
  disposePrivateInstance,
  enterInstance,
  exitInstance,
  type WorkerdRolldownInstance,
} from './workerd-managed-instance';

export interface WorkerdBuildOptions extends InputOptions {
  /**
   * A live instance created by `createInstance()`. The caller keeps ownership
   * and disposes it. Pass exactly one of `instance` or `module`.
   */
  instance?: WorkerdRolldownInstance;
  /**
   * A precompiled threadless Rolldown Wasm module (for workerd: imported under
   * a `CompiledWasm` rule, e.g. `import mod from '@rolldown/browser/workerd/wasm'`).
   * `build()` creates a private instance and disposes it before returning.
   * Pass exactly one of `instance` or `module`.
   */
  module?: WebAssembly.Module | PromiseLike<WebAssembly.Module>;
  /** Output options for the in-memory `generate()` pass. */
  output?: OutputOptions;
}

async function buildWithInstance(
  instance: WorkerdRolldownInstance,
  inputOptions: InputOptions,
  output: OutputOptions,
): Promise<RolldownOutput> {
  enterInstance(instance);
  try {
    const result = await rolldownBuild({ ...inputOptions, output, write: false });
    // Materialize every output field while the instance is still the active
    // binding, then drop the raw binding wrappers: their methods close over the
    // instance's emnapi env, so a retained result would keep its memory alive.
    if (result instanceof RolldownOutputImpl) result.releaseBindings();
    return result;
  } finally {
    exitInstance(instance);
  }
}

/**
 * Record a failed private dispose on the build error without throwing, so the
 * build error is always what `build()` rejects with. The disposal error becomes
 * `cause`; a cause the error already has (a release error, or its own) is kept
 * beside it in an AggregateError. A thrown value that is not an object, or
 * whose `cause` cannot be read or can be neither assigned nor redefined, drops
 * the disposal error.
 * See internal-docs/workerd-managed-instance/implementation.md (invariant 6).
 */
function attachDisposeError(error: unknown, disposeError: unknown): void {
  if (Object(error) !== error) return;
  const target = error as { cause?: unknown };
  try {
    const value =
      target.cause === undefined
        ? disposeError
        : new AggregateError(
            [target.cause, disposeError],
            'Disposing the private workerd Rolldown instance also failed',
          );
    let assigned = false;
    try {
      assigned = Reflect.set(target, 'cause', value);
    } catch {}
    if (!assigned) {
      Object.defineProperty(target, 'cause', {
        value,
        writable: true,
        configurable: true,
        enumerable: false,
      });
    }
  } catch {}
}

/**
 * Bundle once, in memory, with the rollup-style options (`input`, `plugins`,
 * `output`, ...). Builds on the same instance may overlap; a build on another
 * instance rejects while one is active, so share one module-scope instance.
 */
export async function build(options: WorkerdBuildOptions): Promise<RolldownOutput> {
  const { instance, module, output = {}, ...inputOptions } = options ?? {};
  if ((instance === undefined) === (module === undefined)) {
    throw new TypeError('Pass exactly one of `instance` or `module` to build()');
  }
  if (
    (bindingNamespace as { __isWorkerdBindingProxy?: unknown }).__isWorkerdBindingProxy !== true
  ) {
    throw new Error(
      'The workerd build() API is only functional from the bundled @rolldown/browser/workerd ' +
        'entry, where the binding is routed to managed instances. In Node.js or the browser, ' +
        'use the rolldown / @rolldown/browser package APIs instead.',
    );
  }
  // No await before entering: a `dispose()` made right after this call is
  // refused instead of starting first.
  if (instance !== undefined) return buildWithInstance(instance, inputOptions, output);
  const owned = await createInstance(module!);
  let result: RolldownOutput;
  try {
    result = await buildWithInstance(owned, inputOptions, output);
  } catch (error) {
    await disposePrivateInstance(owned).catch((disposeError: unknown) =>
      attachDisposeError(error, disposeError),
    );
    throw error;
  }
  await disposePrivateInstance(owned);
  return result;
}
