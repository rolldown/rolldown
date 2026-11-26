import type { BindingModuleMetaEntry } from '../binding.cjs';

/**
 * The entries that the persistent cache saves for the `meta` of a hook result: one entry for each
 * top-level key. `undefined` when the cache cannot save `meta`.
 *
 * Only a plain object, or an object without a prototype, is supported as `meta`, with the same
 * rules as for an object inside a value otherwise: no getter or setter, no property that is not
 * enumerable or has a symbol key, and it can be extended. The module copies `meta` with
 * `Object.assign`, so another object could give a copy that differs from what the cache saved,
 * for example a getter that returns another value on each read.
 *
 * Each value is saved as JSON, so it must be the same after it is written as JSON and read back.
 * A string, a boolean, `null`, a finite number, an array and a plain object are. Everything that
 * JSON changes or drops is not: a function, a symbol, a `BigInt`, `NaN`, `Infinity`, `-0`, a
 * `Map`, a `Set`, a `Date`, an instance of a class, an object without a prototype, `undefined`
 * inside a value (an item, a hole, or the value of a property), a property with a symbol key, a
 * property that is not enumerable, a getter or a setter, an object that is frozen, sealed or
 * cannot be extended, and an object that is used two times or contains itself. A top-level key
 * that is not well-formed UTF-16 cannot be saved either.
 *
 * `undefined` under a top-level key gets an entry without JSON text. The copy writes `undefined`
 * over an older value of the key, so a reuse must do the same.
 */
export function toSavableMetaEntries(meta: object): BindingModuleMetaEntry[] | undefined {
  try {
    // The copy that the module keeps has the usual prototype anyway, so `meta` itself can also be
    // an object without a prototype.
    const keys = Array.isArray(meta) ? undefined : keysOfPlain(meta, true);
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

/** The `meta` that `toSavableMetaEntries` saved, as the object of a hook result. */
export function metaFromEntriesJson(text: string): Record<string, unknown> {
  return Object.fromEntries(
    (JSON.parse(text) as [string, unknown?][]).map((entry) => [entry[0], entry[1]]),
  );
}

/**
 * Whether `value` is the same after it is written as JSON and read back, as the values under the
 * top-level keys of a saved `meta` must be. The persistent cache also saves the logs of plugins
 * this way.
 */
export function isSavableAsJson(value: unknown): boolean {
  try {
    return isSavable(value, new Set());
  } catch {
    // For example, a value is nested too deeply.
    return false;
  }
}

/**
 * The keys of `value`, or `undefined` when `value` is not a plain object or a plain array: it
 * has another prototype, a property with a symbol key, a property that is not enumerable, a
 * getter or a setter, or it cannot be extended. JSON keeps none of these: a getter becomes the
 * value that it returned once, and a frozen or sealed object becomes a normal one.
 *
 * With `allowNoPrototype`, an object without a prototype counts as plain too.
 */
function keysOfPlain(value: object, allowNoPrototype = false): string[] | undefined {
  const isArray = Array.isArray(value);
  const prototype = Object.getPrototypeOf(value);
  const expected = isArray ? Array.prototype : Object.prototype;
  if (prototype !== expected && !(allowNoPrototype && !isArray && prototype === null)) {
    return undefined;
  }
  if (!Object.isExtensible(value)) {
    return undefined;
  }
  const keys = Object.keys(value);
  // An array has the own property `length`, which is not enumerable.
  if (Reflect.ownKeys(value).length !== keys.length + (isArray ? 1 : 0)) {
    return undefined;
  }
  for (const key of keys) {
    if (!('value' in Object.getOwnPropertyDescriptor(value, key)!)) {
      return undefined;
    }
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
    if (!isSavable(item, seen)) {
      return false;
    }
  }
  return true;
}
