// One SIGINT handler for the whole process. Each command registers its cleanup
// here instead of adding its own listener: Node calls listeners in order, and a
// listener that exits first would skip the cleanups registered after it (a field
// run would then leave its variant cookies in the user's profile).

const cleanups = new Set();
let installed = false;

/** Run `fn` on Ctrl-C (newest first), then exit 130. Returns a function that unregisters it. */
export function onInterrupt(fn) {
  if (!installed) {
    installed = true;
    process.on('SIGINT', async () => {
      for (const cleanup of [...cleanups].reverse()) {
        cleanups.delete(cleanup);
        await Promise.resolve()
          .then(cleanup)
          .catch(() => {});
      }
      process.exit(130);
    });
  }
  cleanups.add(fn);
  return () => cleanups.delete(fn);
}
