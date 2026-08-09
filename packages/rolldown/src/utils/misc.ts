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

// Ported verbatim from Rollup so `emitFile` validation matches Rollup exactly:
//   - rollup/src/utils/relativeId.ts (`isPathFragment`)
//   - rollup/src/utils/path.ts       (`ABSOLUTE_PATH_REGEX`)
// Keep this in sync with the Rust copy `is_path_fragment` in
// `crates/rolldown_common/src/inner_bundler_options/types/filename_template.rs`,
// which the native render/emit checks use.
const ABSOLUTE_PATH_REGEX = /^(?:\/|(?:[A-Za-z]:)?[/\\|])/;

/**
 * Whether `name` is a path fragment — an absolute or relative path. Emitted file
 * names, `[name]` substitutions and file-name patterns can be neither.
 */
export function isPathFragment(name: string): boolean {
  // starting with "/", "./", "../", "C:/", "\", "|", …
  return (
    name[0] === '/' ||
    (name[0] === '.' && (name[1] === '/' || name[1] === '.')) ||
    ABSOLUTE_PATH_REGEX.test(name)
  );
}
