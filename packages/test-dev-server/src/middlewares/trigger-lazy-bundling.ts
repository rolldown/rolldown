import type http from 'node:http';
import type { FullBundleDevEnvironment } from '../environments/full-bundle-dev-environment.js';

type Next = (err?: unknown) => void;

/**
 * Lazy-bundling endpoint, ported from Vite's `triggerLazyBundlingMiddleware`.
 * `/@vite/lazy?id=...&clientId=...` compiles a lazy entry on demand.
 */
export function triggerLazyBundlingMiddleware(env: FullBundleDevEnvironment) {
  return async function lazyBundlingMiddleware(
    req: http.IncomingMessage,
    res: http.ServerResponse,
    next: Next,
  ): Promise<void> {
    if (!req.url?.startsWith('/@vite/lazy?')) {
      next();
      return;
    }

    let params: URLSearchParams;
    try {
      params = new URL(`http://localhost${req.url}`).searchParams;
    } catch {
      next();
      return;
    }

    const moduleId = params.get('id');
    const clientId = params.get('clientId');
    console.log(`Lazy compile request for module ${moduleId} from client ${clientId}`);

    let code: string | undefined;
    try {
      code = await env.triggerLazyBundling(moduleId, clientId);
    } catch (err) {
      res.statusCode = 500;
      res.end('Internal Server Error during lazy compilation');
      console.error('Error handling lazy compile request:', err);
      return;
    }

    if (code == null) {
      next();
      return;
    }
    res.setHeader('Content-Type', 'application/javascript');
    res.end(code);
  };
}
