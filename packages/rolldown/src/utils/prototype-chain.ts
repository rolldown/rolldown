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
