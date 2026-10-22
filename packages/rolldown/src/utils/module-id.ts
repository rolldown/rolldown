// Import attributes are part of the module id: `rawId?attributes=<encoded sorted JSON>`.
// The encoding is byte-identical to Rollup 5's `generateIdByRawIdAndAttributes` and to
// `ImportAttributes::compose_module_id` in `crates/rolldown_common`.
// See internal-docs/import-attributes/implementation.md

/**
 * A module id, or the raw id and import attributes it is built from.
 *
 * @category Plugin APIs
 */
export type UniqueModuleId = string | { rawId: string; attributes?: Record<string, string> };

const ATTRIBUTES_MARKER_LENGTH = '&attributes='.length;

/**
 * Builds the module id of `rawId` imported with `attributes`. Without attributes it is `rawId`
 * itself.
 *
 * @example
 * ```js
 * stringifyModuleId('/src/a.json', { type: 'json' });
 * // → '/src/a.json?attributes=%7B%22type%22%3A%22json%22%7D'
 * ```
 *
 * @category Utilities
 */
export function stringifyModuleId(rawId: string, attributes?: Record<string, string>): string {
  if (!attributes) return rawId;
  const entries = Object.entries(attributes);
  if (entries.length === 0) return rawId;
  const sorted = Object.fromEntries(entries.sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0)));
  return `${rawId}${rawId.includes('?') ? '&' : '?'}attributes=${encodeURIComponent(
    JSON.stringify(sorted),
  )}`;
}

/**
 * Splits a module id into the raw id and the import attributes it was built from.
 *
 * An id that only looks like it has attributes, e.g. `virtual?attributes=foo`, is returned as
 * the raw id with no attributes, because only a suffix that {@linkcode stringifyModuleId}
 * produces exactly is accepted.
 *
 * @example
 * ```js
 * parseModuleId('/src/a.json?attributes=%7B%22type%22%3A%22json%22%7D');
 * // → { rawId: '/src/a.json', attributes: { type: 'json' } }
 * parseModuleId('/src/a.js?raw');
 * // → { rawId: '/src/a.js?raw', attributes: {} }
 * ```
 *
 * @category Utilities
 */
export function parseModuleId(id: string): {
  rawId: string;
  attributes: Record<string, string>;
} {
  const markerIndex = Math.max(id.lastIndexOf('?attributes='), id.lastIndexOf('&attributes='));
  if (markerIndex !== -1) {
    try {
      const rawId = id.slice(0, markerIndex);
      const attributes = JSON.parse(
        decodeURIComponent(id.slice(markerIndex + ATTRIBUTES_MARKER_LENGTH)),
      );
      if (
        typeof attributes === 'object' &&
        attributes !== null &&
        Object.values(attributes).every((value) => typeof value === 'string') &&
        stringifyModuleId(rawId, attributes) === id
      ) {
        return { rawId, attributes };
      }
    } catch {
      // Not encoded attributes, so the id is opaque.
    }
  }
  return { rawId: id, attributes: {} };
}

/** @internal */
export function normalizeUniqueModuleId(id: UniqueModuleId): {
  rawId: string;
  attributes: Record<string, string>;
} {
  return typeof id === 'object'
    ? { rawId: id.rawId, attributes: id.attributes ?? {} }
    : parseModuleId(id);
}
