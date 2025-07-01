export function arraify<T>(value: T | T[]): T[] {
  return Array.isArray(value) ? value : [value];
}

export function isPromiseLike(value: any): value is PromiseLike<any> {
  return (
    value &&
    (typeof value === 'object' || typeof value === 'function') &&
    typeof value.then === 'function'
  );
}

export function unimplemented(info?: string): never {
  if (info) {
    throw new Error(`unimplemented: ${info}`);
  }
  throw new Error('unimplemented');
}

export function unreachable(info?: string): never {
  if (info) {
    throw new Error(`unreachable: ${info}`);
  }
  throw new Error('unreachable');
}
export function unsupported(info: string): never {
  throw new Error(`UNSUPPORTED: ${info}`);
}
export function noop(..._args: any[]): void {}
