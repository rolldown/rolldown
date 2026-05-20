// Compressed reproduction of the ksmth/rolldown-cjs-class-field-repro shape
// linked from #9263: an ESM entry imports a CJS-classified dep whose class
// uses class fields under target es2021. The synthesized `_defineProperty`
// require sits inside the CJS dep's `__commonJSMin` wrap — the fix path the
// runtime-helper boundary covers.
const m = await import('./dist/main.js');
if (!m.ok) {
  throw new Error('class field init failed across ESM-entry + CJS-dep boundary');
}
