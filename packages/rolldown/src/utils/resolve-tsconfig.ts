import { ResolverFactory, type Tsconfig } from '../binding.cjs';

// process is undefined for browser build
if (typeof process === 'object' && process.versions?.pnp) {
  process.env.OXC_RESOLVER_YARN_PNP = '1';
}

/**
 * Cache for tsconfig resolution to avoid redundant file system operations.
 *
 * The cache reuses the resolver's internal filesystem and tsconfig caches.
 * When transforming multiple files in the same project, repeated tsconfig
 * lookups avoid redundant filesystem work.
 * Pass a tsconfig path to use that configuration for every lookup. When the
 * path is omitted, the nearest tsconfig is discovered automatically.
 *
 * @category Utilities
 * @experimental
 */
export class TsconfigCache {
  /** @internal */
  readonly resolver: ResolverFactory;

  constructor(pathToTsconfig?: string) {
    this.resolver = new ResolverFactory({
      tsconfig:
        pathToTsconfig !== undefined ? { configFile: pathToTsconfig, references: 'auto' } : 'auto',
    });
  }

  /** Clear cached filesystem and tsconfig data. */
  clear(): void {
    this.resolver.clearCache();
  }
}

/**
 * The result of resolving a tsconfig for a given file.
 *
 * @category Utilities
 * @experimental
 */
export interface ResolveTsconfigResult {
  /** The resolved tsconfig object. */
  tsconfig: Tsconfig;
  /** The file paths of the referenced tsconfigs. */
  tsconfigFilePaths: string[];
}

/**
 * Resolve the tsconfig for a given file asynchronously.
 *
 * Note: This function can be slower than `resolveTsconfigSync` due to the overhead of spawning a thread.
 *
 * @param filename The path to the file for which to resolve the tsconfig.
 * @param cache An optional TsconfigCache instance.
 * @returns A promise that resolves to the tsconfig result or null if not found.
 *
 * @category Utilities
 * @experimental
 */
export async function resolveTsconfig(
  filename: string,
  cache: TsconfigCache | null = new TsconfigCache(),
): Promise<ResolveTsconfigResult | null> {
  return cache.resolver.findTsconfigAsync(filename);
}

/**
 * Resolve the tsconfig for a given file.
 *
 * @param filename The path to the file for which to resolve the tsconfig.
 * @param cache An optional TsconfigCache instance.
 * @returns The tsconfig result or null if not found.
 *
 * @category Utilities
 * @experimental
 */
export function resolveTsconfigSync(
  filename: string,
  cache: TsconfigCache | null = new TsconfigCache(),
): ResolveTsconfigResult | null {
  return cache.resolver.findTsconfigSync(filename);
}
