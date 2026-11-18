import { getStartContext } from './server.js';
var __exportAll = (all) => {
  const target = {};
  for (const name in all) Object.defineProperty(target, name, { get: all[name], enumerable: true });
  return target;
};
export const getRouter = () => `router(${getStartContext()})`;
export { __exportAll };
