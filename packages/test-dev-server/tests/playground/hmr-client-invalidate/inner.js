export const inner = 'inner-v1';

// Self-accepts, but its accept callback always delegates: the fully
// client-side `hot.invalidate()` re-walks from this module's importers.
import.meta.hot?.accept(() => {
  import.meta.hot.invalidate();
});
