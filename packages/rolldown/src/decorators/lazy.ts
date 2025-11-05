import { LAZY_FIELDS_KEY } from '../types/plain-object-like';

/**
 * Decorator that marks a getter as lazy-evaluated and cached.
 *
 * **What "lazy" means here:**
 * 1. Data is lazily fetched from Rust bindings only when the property is accessed (not eagerly on construction)
 * 2. Once fetched, the data is cached for subsequent accesses (performance optimization)
 * 3. Despite being a getter, it behaves like a plain object property (enumerable, appears in Object.keys())
 *
 * **Important**: Properties decorated with `@lazyProp` are defined as own enumerable
 * properties on each instance (not on the prototype). This ensures they:
 * - Appear in Object.keys() and Object.getOwnPropertyNames()
 * - Are included in object spreads ({...obj})
 * - Are enumerable in for...in loops
 *
 * Classes using this decorator must extend `PlainObjectLike` base class.
 *
 * @example
 * ```typescript
 * class MyClass extends PlainObjectLike {
 *   @lazyProp
 *   get expensiveValue() {
 *     return someExpensiveComputation();
 *   }
 * }
 * ```
 */
export function lazyProp(
  target: any,
  propertyKey: string,
  descriptor: PropertyDescriptor,
): PropertyDescriptor {
  // Ensure the class constructor has a lazy fields registry
  if (!target.constructor[LAZY_FIELDS_KEY]) {
    target.constructor[LAZY_FIELDS_KEY] = new Map<string, () => any>();
  }

  // Store the original getter function
  // eslint-disable-next-line typescript-eslint/unbound-method
  const originalGetter = descriptor.get!;
  target.constructor[LAZY_FIELDS_KEY].set(propertyKey, originalGetter);

  // Return a non-enumerable descriptor for the prototype
  // This ensures the property won't show up when enumerating the prototype,
  // and will be shadowed by the own property defined in setupLazyProperties()
  return {
    enumerable: false,
    configurable: true,
  };
}
