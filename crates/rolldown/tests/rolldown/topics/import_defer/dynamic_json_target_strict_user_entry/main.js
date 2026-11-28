const entries = (ns) => Object.keys(ns).map((key) => [key, ns[key]]);

export async function run() {
  // `import()` comes first, so no deferred access evaluates `data.json` before it.
  const eager = await import('./data.json');
  const eagerEntries = entries(eager);
  const deferred = await import.defer('./data.json');
  const eagerAgain = await import('./data.json');
  return {
    eager: eagerEntries,
    deferred: entries(deferred),
    sameEager: eager === eagerAgain,
    distinctDeferred: deferred !== eager,
  };
}
