// Workerd bundles alias `src/binding.cjs` to this module, so the pipeline runs
// against a managed per-instance threadless WASI binding instead of a
// process-global native addon. Module evaluation must stay side-effect free:
// every export defers to the currently entered instance.
//
// The binding exports themselves are not listed here: while bundling,
// `aliasWorkerdPipelineModules` in `build.ts` appends one
// `export const X = lazyExport('X')` per name in the `napi-rs-artifact-metadata`
// header of `src/rolldown-binding.wasip1.cjs`, minus the names it skips.
import type { BindingRuntimeCapabilities } from './binding.cjs';

type BindingExports = Record<PropertyKey, unknown>;

let activeExports: BindingExports | undefined;
let activeRefs = 0;
const cachedEnums = new Map<string, object>();

/** @internal Marker checked by `workerd-build.ts` to detect workerd bundles. */
export const __isWorkerdBindingProxy = true;

/** @internal Route the pipeline's binding calls to this instance's exports. */
export function __enterWorkerdBinding(exports: object): void {
  if (exports === null || (typeof exports !== 'object' && typeof exports !== 'function')) {
    throw new TypeError('A workerd Rolldown instance exports object is required');
  }
  if (activeExports !== undefined && activeExports !== exports) {
    throw new Error(
      'Another workerd Rolldown instance is currently active in this module; ' +
        'finish or close builds on it first',
    );
  }
  activeExports = exports as BindingExports;
  activeRefs += 1;
}

/** @internal Release one `__enterWorkerdBinding` reference. */
export function __exitWorkerdBinding(exports: object): void {
  if (activeExports === undefined || activeExports !== exports) return;
  activeRefs -= 1;
  if (activeRefs <= 0) {
    activeRefs = 0;
    activeExports = undefined;
  }
}

function inactiveBindingError(name: string): Error {
  return new Error(
    `Rolldown workerd binding '${name}' was used outside an active build; ` +
      `call build() with a live workerd Rolldown instance`,
  );
}

function req(name: string): any {
  const active = activeExports;
  const value = active === undefined ? undefined : active[name];
  if (value === undefined) {
    throw inactiveBindingError(name);
  }
  return value;
}

/**
 * @internal A stand-in for binding export `name` that forwards every use to
 * the active instance. One shape serves functions, classes and enum objects:
 * the `function` target makes both `apply` and `construct` reachable, and its
 * writable `prototype` lets the `get` trap return the real one, so `instanceof`
 * works while a build is active.
 */
export function lazyExport(name: string): any {
  const shadow = function () {};
  Object.defineProperty(shadow, 'name', { value: name });
  return new Proxy(shadow, {
    apply: (_target, _thisArg, args) => Reflect.apply(req(name), undefined, args),
    construct: (_target, args) => Reflect.construct(req(name), args),
    get: (target, property, receiver) => {
      let source = cachedEnums.get(name);
      if (source === undefined) {
        if (activeExports === undefined && (property === 'name' || property === 'prototype')) {
          // Keep harmless diagnostic reads (and `instanceof` fallbacks, which
          // read `prototype`) from throwing while no instance is active.
          return Reflect.get(target, property, receiver);
        }
        const value = req(name);
        if (typeof value !== 'object') return Reflect.get(value, property);
        // Enum objects are artifact constants; cache the first successful read.
        source = value as object;
        cachedEnums.set(name, source);
      }
      return Reflect.get(source, property);
    },
    has: (_target, property) => Reflect.has(req(name), property),
  });
}

// Matches `__napiBindingTarget` in the generated threadless WASI loader this
// proxy stands in for; `@napi-rs/cli` reports the flavor's `platformArchABI`,
// which is not the same spelling as the Rust `getRuntimeCapabilities().target`.
export const __napiBindingTarget = 'wasm32-wasip1';

// Report used while no managed instance is active, so a capability read before
// the first managed instantiation is safe in workerd bundles. Matches
// `async_runtime.rs get_runtime_capabilities()` for wasm32-wasip1.
const STATIC_THREADLESS_CAPABILITIES: BindingRuntimeCapabilities = Object.freeze({
  devSupported: false,
  flavor: 'CurrentThread',
  target: 'wasi',
  threads: false,
  wasi: true,
  watchSupported: false,
});

export function getRuntimeCapabilities(): BindingRuntimeCapabilities {
  const active = activeExports;
  if (active !== undefined) {
    const reporter = active['getRuntimeCapabilities'];
    if (typeof reporter === 'function') {
      return Reflect.apply(reporter, undefined, []) as BindingRuntimeCapabilities;
    }
  }
  return STATIC_THREADLESS_CAPABILITIES;
}
