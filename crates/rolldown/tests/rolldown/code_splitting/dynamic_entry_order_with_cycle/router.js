import { getContext } from './server.js';

// `var`, so an execution order that runs this module too late leaves the call
// site with `undefined` rather than a TDZ error.
var toNamespace = (all) => {
  const target = {};
  for (const key in all) {
    Object.defineProperty(target, key, { value: all[key], enumerable: true });
  }
  return target;
};

// Deferred: the other half of the cycle is only read when this is called.
export const getRouter = () => `router(${getContext()})`;

export { toNamespace };
