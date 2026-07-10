// Computed key: bundler can't know which export is read, must keep all.
const k = globalThis.k;
import('./lib.js').then((ns) => {
  const { [k]: x } = ns;
  console.log(x);
});
