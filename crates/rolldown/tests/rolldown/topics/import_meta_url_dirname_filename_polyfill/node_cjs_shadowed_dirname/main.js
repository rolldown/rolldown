import assert from 'node:assert';

// `import.meta.dirname` is rewritten to the bare ambient `__dirname`. A
// hoisted local of the same name must not capture that reference: the local
// is still undefined when the rewritten expression runs, silently yielding
// `undefined` instead of the output directory. Every value is read twice so
// nothing is inlined past the shadowing declaration.
function init() {
  const dir = import.meta.dirname;
  var __dirname = 'SENTINEL';
  return [dir, dir, __dirname, __dirname];
}

const [dir, , dirname] = init();
assert.equal(typeof dir, 'string');
assert.ok(!dir.includes('SENTINEL'));
assert.equal(dirname, 'SENTINEL');
