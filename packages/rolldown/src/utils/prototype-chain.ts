const MAX_PROTOTYPE_CHAIN_DEPTH = 256;

// See internal-docs/async-context/implementation.md.
export function findPropertyDescriptorInPrototypeChain(
  value: object,
  key: PropertyKey,
  operation: string,
): PropertyDescriptor | undefined {
  let current: object | null = value;
  const visited = new Set<object>();
  let depth = 0;
  while (current) {
    if (visited.has(current)) {
      throw new TypeError(`Prototype cycle detected while ${operation}`);
    }
    if (depth >= MAX_PROTOTYPE_CHAIN_DEPTH) {
      throw new TypeError(
        `Prototype chain exceeded ${MAX_PROTOTYPE_CHAIN_DEPTH} objects while ${operation}`,
      );
    }
    visited.add(current);
    depth += 1;

    const descriptor = Reflect.getOwnPropertyDescriptor(current, key);
    if (descriptor) return descriptor;
    current = Reflect.getPrototypeOf(current);
  }
}

export function hasCallableThenWithoutInvokingAccessor(value: object): boolean {
  const descriptor = findPropertyDescriptorInPrototypeChain(
    value,
    'then',
    'checking a repeated thenable resolution',
  );
  if (!descriptor) return false;
  if ('value' in descriptor) return typeof descriptor.value === 'function';

  // The same object already produced a callable `then` on this resolution
  // path. Treat an accessor that still exists as the same cycle without
  // invoking user code again. Deleting it still permits mutable self-resolution.
  return typeof descriptor.get === 'function';
}
