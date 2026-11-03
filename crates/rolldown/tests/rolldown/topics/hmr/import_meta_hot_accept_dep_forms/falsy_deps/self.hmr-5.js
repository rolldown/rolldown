globalThis.falsy_depsSelfRuns = (globalThis.falsy_depsSelfRuns ?? 0) + 1;
export const count = 1;

import.meta.hot.accept(undefined, () => {
  globalThis.falsy_depsCallbackRan = true;
});
