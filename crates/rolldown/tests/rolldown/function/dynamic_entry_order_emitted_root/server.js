import { __exportAll, getRouter } from './router.js';
export function getStartContext() {
  return 'ctx';
}
async function loadRoutes() {
  const a = await import('./routeA.js');
  return [a.A(), getRouter()];
}
export const server_exports = /* @__PURE__ */ __exportAll({ default: () => loadRoutes });
