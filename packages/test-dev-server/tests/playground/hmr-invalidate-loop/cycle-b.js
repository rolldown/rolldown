// Completes the import cycle with cycle-a.
import './cycle-a.js';

// The dep-accepting boundary whose callback delegates via `invalidate()`.
// Every apply of the boundary re-runs this callback, and nothing dedups the
// repeated invalidate from the same module.
import.meta.hot?.accept('./cycle-a.js', () => {
  import.meta.hot.invalidate();
});
