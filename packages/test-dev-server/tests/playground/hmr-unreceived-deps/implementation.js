globalThis.__implementationExecutions = (globalThis.__implementationExecutions ?? 0) + 1;

export function decorate(value) {
  return `[${value}]`;
}
