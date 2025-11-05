const LAZY_FIELDS_KEY: unique symbol = Symbol('__lazy_fields__');

/**
 * Base class for classes that use `@lazyProp` decorated properties.
 *
 * **Design Pattern in Rolldown:**
 * This is a common pattern in Rolldown due to its three-layer architecture:
 * TypeScript API → NAPI Bindings → Rust Core
 *
 * **Why we use getters:**
 * For performance - to lazily fetch data from Rust bindings only when needed,
 * rather than eagerly fetching all data during object construction.
 *
 * **The problem:**
 * Getters defined on class prototypes are non-enumerable by default, which breaks:
 * - Object spread operators ({...obj})
 * - Object.keys() and similar methods
 * - Standard JavaScript object semantics
 *
 * **The solution:**
 * This base class automatically converts `@lazyProp` decorated getters into
 * own enumerable getters on each instance during construction.
 *
 * **Result:**
 * Objects get both lazy-loading performance benefits AND plain JavaScript object behavior.
 *
 * @example
 * ```typescript
 * class MyClass extends PlainObjectLike {
 *   @lazyProp
 *   get myProp() {
 *     return fetchFromRustBinding();
 *   }
 * }
 * ```
 */
export class PlainObjectLike {
  constructor() {
    setupLazyProperties(this);
  }
}

/**
 * Set up lazy properties as own getters on an instance.
 * This is called automatically by the `PlainObjectLike` base class constructor.
 *
 * @param instance - The instance to set up lazy properties on
 * @internal
 */
function setupLazyProperties(instance: any): void {
  const lazyFields = instance.constructor[LAZY_FIELDS_KEY];
  if (!lazyFields) return;

  for (const [propertyKey, originalGetter] of lazyFields.entries()) {
    let cachedValue: any;
    let hasValue = false;

    // Define an own enumerable getter that caches the result
    Object.defineProperty(instance, propertyKey, {
      get(this: any) {
        if (!hasValue) {
          cachedValue = originalGetter.call(this);
          hasValue = true;
        }
        return cachedValue;
      },
      enumerable: true,
      configurable: true,
    });
  }
}

/**
 * Get all lazy field names from a class instance.
 *
 * @param instance - Instance to inspect
 * @returns Set of lazy property names
 */
export function getLazyFields(instance: any): Set<string> {
  const lazyFields = instance.constructor[LAZY_FIELDS_KEY];
  return lazyFields ? new Set(lazyFields.keys()) : new Set();
}

/**
 * @internal
 * Export the symbol for use by the lazyProp decorator
 */
export { LAZY_FIELDS_KEY };
