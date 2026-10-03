import { toNamespace, getRouter } from './router.js';

export function getContext() {
  return 'context';
}

async function loadRoutes() {
  const routes = await import('./routes.js');
  return [routes.route(), getRouter()];
}

// Runs while this module's body is evaluated, so `toNamespace` must already be
// assigned by the time the cycle reaches here.
export const serverExports = toNamespace({
  name: 'server',
  loadRoutes,
});
