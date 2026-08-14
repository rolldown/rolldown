import * as Devtools from './tan-stack-router-devtools.js';

export const TanStackRouterDevtools =
  process.env.NODE_ENV !== 'development'
    ? function () {
        return null;
      }
    : Devtools.TanStackRouterDevtools;

export const TanStackRouterDevtoolsInProd = Devtools.TanStackRouterDevtools;
