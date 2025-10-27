const LAZY_FIELDS_KEY = Symbol('__lazy_fields__');
const LAZY_CACHE_PREFIX = '__cached_';

/**
 * Legacy decorator that makes a getter lazy-evaluated and cached.
 * Also auto-registers the field for batch prefetching.
 *
 * @example
 * ```typescript
 * class MyClass {
 *   @lazy
 *   get expensiveValue() {
 *     return someExpensiveComputation();
 *   }
 * }
 * ```
 */
export function lazy(
  target: any,
  propertyKey: string,
  descriptor: PropertyDescriptor,
): PropertyDescriptor {
  // Ensure the class constructor has a lazy fields registry
  if (!target.constructor[LAZY_FIELDS_KEY]) {
    target.constructor[LAZY_FIELDS_KEY] = new Set<string>();
  }
  target.constructor[LAZY_FIELDS_KEY].add(propertyKey);

  // eslint-disable-next-line typescript-eslint/unbound-method
  const originalGetter = descriptor.get!;
  const cacheKey = LAZY_CACHE_PREFIX + propertyKey;

  descriptor.get = function(this: any) {
    if (!(cacheKey in this)) {
      this[cacheKey] = originalGetter.call(this);
    }
    return this[cacheKey];
  };

  return descriptor;
}

/**
 * Get all lazy field names from a class instance.
 *
 * @param instance - Instance to inspect
 * @returns Set of lazy property names
 */
export function getLazyFields(instance: any): Set<string> {
  return instance.constructor[LAZY_FIELDS_KEY] || new Set();
}
