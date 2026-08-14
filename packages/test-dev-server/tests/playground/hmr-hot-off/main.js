import './target.js';

// Ports Vite's `hot.off` behavior (Vite tests it via plugin custom events; FBM has no
// upstream channel, so the built-in `vite:beforeUpdate` event exercises the same
// listener add/remove machinery).
const w = /** @type {any} */ (window);
const removed = () => {
  w.__removedCalls = (w.__removedCalls ?? 0) + 1;
};
import.meta.hot?.on('vite:beforeUpdate', removed);
import.meta.hot?.off('vite:beforeUpdate', removed);

const kept = () => {
  w.__keptCalls = (w.__keptCalls ?? 0) + 1;
};
import.meta.hot?.on('vite:beforeUpdate', kept);
