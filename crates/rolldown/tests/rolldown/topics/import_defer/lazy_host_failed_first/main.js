import { log } from './log.js';

// No top-level await in this file. Then `d.js` stays in its own dynamic entry chunk, and the chunk
// of `importer.js` imports that chunk statically.
export const result = (async () => {
  let err1, err2;
  await import('./d.js').catch((e) => {
    err1 = e;
  });
  // The chunk of `d.js` did not throw when it loaded, so this import succeeds.
  const { ns } = await import('./importer.js');
  try {
    ns.value;
  } catch (e) {
    err2 = e;
  }
  return [err1?.message, err1 === err2];
})();
export { log };
