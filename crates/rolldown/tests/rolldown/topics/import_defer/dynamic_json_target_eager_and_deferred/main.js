// The static import keeps `local.json` in this chunk. `first.json` and `second.json` are each in
// their own chunk.
import defer * as localNs from './local.json';

const entries = (ns) => Object.keys(ns).map((key) => [key, ns[key]]);
const compare = (eager, deferred, eagerAgain) => ({
  eager: entries(eager),
  deferred: entries(deferred),
  sameEager: eager === eagerAgain,
  distinctDeferred: deferred !== eager,
});

export async function run() {
  // `import()` first, then `import.defer()`.
  const firstEager = await import('./first.json');
  const firstDeferred = await import.defer('./first.json');
  const firstEagerAgain = await import('./first.json');
  // `import.defer()` first, then `import()`.
  const secondDeferred = await import.defer('./second.json');
  const secondEager = await import('./second.json');
  const secondEagerAgain = await import('./second.json');
  const localEager = await import('./local.json');
  const localDeferred = await import.defer('./local.json');
  const localEagerAgain = await import('./local.json');
  return {
    first: compare(firstEager, firstDeferred, firstEagerAgain),
    second: compare(secondEager, secondDeferred, secondEagerAgain),
    local: {
      ...compare(localEager, localDeferred, localEagerAgain),
      static: localNs === localDeferred,
    },
  };
}
