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

const ABSOLUTE_PATH_REGEX = /^(?:\/|(?:[A-Za-z]:)?[/\\|])/;

// Rollup's `isPathFragment`: names starting with "/", ".." , "./" or a
// Windows drive prefix are path fragments and not valid file names.
export function isPathFragment(name: string): boolean {
  return (
    name[0] === '/' ||
    (name[0] === '.' && (name[1] === '/' || name[1] === '.')) ||
    ABSOLUTE_PATH_REGEX.test(name)
  );
}
