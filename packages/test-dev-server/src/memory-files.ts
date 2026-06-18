import nodeCrypto from 'node:crypto';

/**
 * In-memory output store, ported from Vite full-bundle mode
 * (`packages/vite/src/node/server/environments/fullBundleEnvironment.ts`).
 *
 * In browser platform the dev engine runs with `watch.skipWrite: true`, so the
 * full bundle and HMR patches never hit disk — they live here and are served by
 * the memory-files / index-html middlewares (mirroring Vite's
 * `memoryFilesMiddleware` + `indexHtmlMiddleware`).
 */
export type MemoryFile = {
  source: string | Uint8Array;
  etag?: string;
};

export class MemoryFiles {
  #files = new Map<string, MemoryFile | (() => MemoryFile)>();

  get size(): number {
    return this.#files.size;
  }

  get(file: string): MemoryFile | undefined {
    const result = this.#files.get(file);
    if (result === undefined) {
      return undefined;
    }
    // Lazily materialize (and memoize) so etag/source are only computed for
    // files that are actually requested.
    if (typeof result === 'function') {
      const content = result();
      this.#files.set(file, content);
      return content;
    }
    return result;
  }

  set(file: string, content: MemoryFile | (() => MemoryFile)): void {
    this.#files.set(file, content);
  }

  has(file: string): boolean {
    return this.#files.has(file);
  }

  clear(): void {
    this.#files.clear();
  }
}

/** Weak etag over the content, matching Vite's `etag` weak-mode usage. */
export function weakEtag(source: string | Uint8Array): string {
  const hash = nodeCrypto.createHash('sha1').update(source).digest('base64');
  const len = typeof source === 'string' ? Buffer.byteLength(source) : source.length;
  return `W/"${len.toString(16)}-${hash.slice(0, 27)}"`;
}

const MIME_BY_EXT: Record<string, string> = {
  '.js': 'application/javascript',
  '.mjs': 'application/javascript',
  '.cjs': 'application/javascript',
  '.json': 'application/json',
  '.css': 'text/css',
  '.html': 'text/html',
  '.map': 'application/json',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.ico': 'image/x-icon',
  '.wasm': 'application/wasm',
  '.txt': 'text/plain',
};

export function contentTypeFor(filePath: string): string | undefined {
  const dot = filePath.lastIndexOf('.');
  if (dot < 0) return undefined;
  return MIME_BY_EXT[filePath.slice(dot).toLowerCase()];
}
