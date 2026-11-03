import {
  resolveTsconfig as originalResolveTsconfig,
  type BindingTsconfigResult,
  TsconfigCache as OriginalTsconfigCache,
} from '../binding.cjs';

// process is undefined for browser build
const yarnPnp = typeof process === 'object' && !!process.versions?.pnp;

/**
 * Cache for tsconfig resolution to avoid redundant file system operations.
 *
 * The cache stores resolved tsconfig configurations keyed by their file paths.
 * When transforming multiple files in the same project, tsconfig lookups are
 * deduplicated, improving performance.
 * Pass a tsconfig path to use that configuration for every lookup. When the
 * path is omitted, the nearest tsconfig is discovered automatically.
 *
 * @category Utilities
 * @experimental
 */
export class TsconfigCache extends OriginalTsconfigCache {
  constructor(pathToTsconfig?: string) {
    super(yarnPnp, pathToTsconfig);
  }
}

/** @hidden This is only expected to be used by Vite */
export function resolveTsconfig(
  filename: string,
  cache?: TsconfigCache | null,
): BindingTsconfigResult | null {
  return originalResolveTsconfig(filename, cache, yarnPnp);
}
