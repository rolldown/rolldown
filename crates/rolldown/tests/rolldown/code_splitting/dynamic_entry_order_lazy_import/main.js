const routes = {
  settings: () => import('./settings.js'),
  home: () => import('./home.js'),
};
export const page = (await routes.home()).page;
export { routes };
