import type http from 'node:http';
import type { FullBundleDevEnvironment } from '../environments/full-bundle-dev-environment.js';
import { generateFallbackHtml } from '../fallback-html.js';
import type { MemoryFile } from '../memory-files.js';

type Next = (err?: unknown) => void;

/**
 * Serve index.html from the in-memory store, or the "Bundling in progress"
 * spinner when output is not ready / a stale regeneration was just triggered.
 * Ported from Vite's `indexHtmlMiddleware` + `generateFallbackHtml`.
 */
export function indexHtmlMiddleware(env: FullBundleDevEnvironment) {
  return async function indexHtmlMiddleware(
    req: http.IncomingMessage,
    res: http.ServerResponse,
    next: Next,
  ): Promise<void> {
    const cleanedUrl = (req.url ?? '').split('?', 1)[0];
    if (cleanedUrl !== '/' && !cleanedUrl.endsWith('.html')) {
      next();
      return;
    }
    const filePath = cleanedUrl === '/' ? 'index.html' : decodeURIComponent(cleanedUrl).slice(1);

    let file: MemoryFile | undefined = env.memoryFiles.get(filePath);
    // The html isn't in memory but other output is → it genuinely doesn't exist.
    if (!file && env.memoryFiles.size !== 0) {
      next();
      return;
    }
    // Serve the spinner while a stale regeneration runs, or before any output
    // exists. It auto-reloads when the server broadcasts `hmr:reload`.
    if ((await env.triggerBundleRegenerationIfStale()) || file === undefined) {
      file = { source: generateFallbackHtml() };
    }

    const html = typeof file.source === 'string' ? file.source : Buffer.from(file.source);
    res.setHeader('Content-Type', 'text/html');
    if (file.etag) {
      res.setHeader('Etag', file.etag);
    }
    res.end(html);
  };
}
