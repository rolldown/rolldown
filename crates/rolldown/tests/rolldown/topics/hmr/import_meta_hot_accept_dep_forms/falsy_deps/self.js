globalThis.falsy_depsSelfRuns = (globalThis.falsy_depsSelfRuns ?? 0) + 1;
export const count = 0;

// A falsy first argument self-accepts, and Vite's client never calls the callback.
import.meta.hot.accept(undefined, () => {
  globalThis.falsy_depsCallbackRan = true;
});
