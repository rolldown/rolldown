import type http from 'node:http';
import type { FullBundleDevEnvironment } from '../environments/full-bundle-dev-environment.js';

type Next = (err?: unknown) => void;

/**
 * Test-only `/_dev/status` endpoint (no Vite equivalent). The e2e harness polls
 * it to await builds (`buildSeq`) and module registrations
 * (`moduleRegistrationSeq`), and to read bundle freshness.
 */
export function statusMiddleware(env: FullBundleDevEnvironment) {
  return async function statusMiddleware(
    req: http.IncomingMessage,
    res: http.ServerResponse,
    next: Next,
  ): Promise<void> {
    if (req.url !== '/_dev/status') {
      next();
      return;
    }
    const status = await env.getStatus();
    res.setHeader('Content-Type', 'application/json');
    res.end(JSON.stringify(status));
  };
}
