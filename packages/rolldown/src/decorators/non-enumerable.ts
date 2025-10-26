/**
 * Decorator that makes a property or method non-enumerable.
 * This hides the property from enumeration (e.g., Object.keys(), for...in loops).
 *
 * @example
 * ```typescript
 * class MyClass {
 *   @nonEnumerable
 *   hiddenMethod() {
 *     return 'This method will not show up in Object.keys()';
 *   }
 * }
 * ```
 */
export function nonEnumerable(
  target: any,
  propertyKey: string,
  descriptor: PropertyDescriptor,
): PropertyDescriptor {
  descriptor.enumerable = false;
  return descriptor;
}
