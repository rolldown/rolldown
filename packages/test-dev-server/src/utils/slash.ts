/**
 * Convert `\` separators to `/` — a mirror of vite's internal `slash`.
 * Module-graph lookup keys must be in vite's normalized form (forward
 * slashes), but playground configs cannot import vite itself, so keys
 * built with `path.join` go through this instead.
 */
export function slash(p: string): string {
  return p.replaceAll('\\', '/');
}
