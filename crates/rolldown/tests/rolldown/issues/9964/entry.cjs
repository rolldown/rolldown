// CommonJS entry that `require`s the ESM consumer => wraps `story.js` in `__esm`
// (WrapKind::Esm). The `require` here is the only wrap trigger (no
// strictExecutionOrder). With the bug, this `require` throws `ReferenceError: eg
// is not defined` at module init, before `run` is ever called.
const { run } = require('./story.js');

if (run() !== 'ok') {
  throw new Error('REPRO FAILED: run() returned ' + run());
}
