import type { BindingModuleMetaEntry } from '../binding.cjs';

/**
 * Whether `meta` is a plain object without properties. A hook wrapper passes a new `{}` when
 * the hook returned no `meta`, and the persistent cache has nothing to save for it.
 */
export function isEmptyPlainObject(meta: unknown): boolean {
  return (
    typeof meta === 'object' &&
    meta !== null &&
    Object.getPrototypeOf(meta) === Object.prototype &&
    Reflect.ownKeys(meta).length === 0
  );
}

/**
 * The entries that the persistent cache saves for the `meta` that a hook returned: one entry
 * for each top-level key. `undefined` when the cache cannot save `meta`.
 *
 * The cache saves each value as JSON. So `meta` must be the same after it is written as JSON
 * and read back. A string, a boolean, `null`, a finite number, an array and a plain object are.
 * Everything that JSON changes or drops is not: a function, a symbol, a `BigInt`, `NaN`,
 * `Infinity`, `-0`, a `Map`, a `Set`, a `Date`, an instance of a class, an object without a
 * prototype, an `undefined` or a hole in an array, a property with a symbol key, a property
 * that is not enumerable, and an object that is used two times or contains itself. A top-level
 * key that is not well-formed UTF-16 cannot be saved either.
 *
 * `meta` itself must be a plain object too. When the module has no `meta` yet,
 * `updateModuleOption` keeps the object of the hook as the `meta` of the module.
 *
 * `undefined` as the value of a property is handled in two ways:
 * - Under a top-level key, it gets an entry without JSON text. `updateModuleOption` writes
 *   `undefined` over an older value of the key, so a reuse must do the same.
 * - Deeper inside a value, the property is left out, as JSON does. This is the one loss that
 *   is allowed.
 */
export function toSavableMetaEntries(meta: unknown): BindingModuleMetaEntry[] | undefined {
  try {
    if (typeof meta !== 'object' || meta === null || Array.isArray(meta)) {
      return undefined;
    }
    const keys = keysOfPlain(meta);
    if (keys === undefined) {
      return undefined;
    }
    const seen = new Set<object>([meta]);
    const entries: BindingModuleMetaEntry[] = [];
    for (const key of keys) {
      // Rust gets the key as UTF-8, which cannot hold one half of a surrogate pair. The JSON
      // text of a value can: `JSON.stringify` writes the half as an escape.
      if (!key.isWellFormed()) {
        return undefined;
      }
      const value = (meta as Record<string, unknown>)[key];
      if (value === undefined) {
        entries.push({ key });
      } else if (isSavable(value, seen)) {
        entries.push({ key, json: JSON.stringify(value) });
      } else {
        return undefined;
      }
    }
    return entries;
  } catch {
    // For example, a value is nested too deeply.
    return undefined;
  }
}

/**
 * The keys of `value`, or `undefined` when `value` is not a plain object or a plain array: it
 * has another prototype, a property with a symbol key, or a property that is not enumerable.
 */
function keysOfPlain(value: object): string[] | undefined {
  const isArray = Array.isArray(value);
  if (Object.getPrototypeOf(value) !== (isArray ? Array.prototype : Object.prototype)) {
    return undefined;
  }
  const keys = Object.keys(value);
  // An array has the own property `length`, which is not enumerable.
  if (Reflect.ownKeys(value).length !== keys.length + (isArray ? 1 : 0)) {
    return undefined;
  }
  return keys;
}

/**
 * `seen` holds every object of the `meta` that was visited before `value`. JSON writes an
 * object that is used two times as two objects, and it cannot write an object that contains
 * itself.
 */
function isSavable(value: unknown, seen: Set<object>): boolean {
  switch (typeof value) {
    case 'string':
    case 'boolean':
      return true;
    case 'number':
      return Number.isFinite(value) && !Object.is(value, -0);
    case 'object':
      break;
    default:
      return false;
  }
  if (value === null) {
    return true;
  }
  if (seen.has(value)) {
    return false;
  }
  seen.add(value);
  const keys = keysOfPlain(value);
  if (keys === undefined) {
    return false;
  }
  if (Array.isArray(value)) {
    // With as many keys as items, every item is set and there is no other property.
    if (keys.length !== value.length) {
      return false;
    }
    for (let i = 0; i < value.length; i++) {
      if (!isSavable(value[i], seen)) {
        return false;
      }
    }
    return true;
  }
  for (const key of keys) {
    const item = (value as Record<string, unknown>)[key];
    if (item !== undefined && !isSavable(item, seen)) {
      return false;
    }
  }
  return true;
}
