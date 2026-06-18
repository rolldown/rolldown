import type http from 'node:http';
import type { FullBundleDevEnvironment } from '../environments/full-bundle-dev-environment.js';
import { contentTypeFor } from '../memory-files.js';

type Next = (err?: unknown) => void;

/**
 * Serve a built asset / HMR patch from the in-memory store, ported from Vite's
 * `memoryFilesMiddleware`. `.html` is left to the index-html middleware.
 */
export function memoryFilesMiddleware(env: FullBundleDevEnvironment) {
  return function memoryFilesMiddleware(
    req: http.IncomingMessage,
    res: http.ServerResponse,
    next: Next,
  ): void {
    const cleanedUrl = (req.url ?? '').split('?', 1)[0];
    if (cleanedUrl.endsWith('.html')) {
      next();
      return;
    }
    const filePath = decodeURIComponent(cleanedUrl).slice(1); // remove leading /
    const file = env.memoryFiles.get(filePath);
    if (!file) {
      next();
      return;
    }
    if (file.etag) {
      if (req.headers['if-none-match'] === file.etag) {
        res.statusCode = 304;
        res.end();
        return;
      }
      res.setHeader('Etag', file.etag);
    }
    const mime = contentTypeFor(filePath);
    if (mime) {
      res.setHeader('Content-Type', mime);
    }
    res.end(file.source);
  };
}
