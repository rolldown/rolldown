import nodeAssert from 'node:assert/strict';

async function main() {
  const exports = await import('./lib.js');

  nodeAssert.deepEqual(Object.keys(exports).sort(), ['default', 'parse']);
  nodeAssert.strictEqual(exports.parse, 'parse', 'Expected export exists and is correct');
  nodeAssert.strictEqual(
    exports.default.parse,
    'parse',
    'Target has __esModule but no own default export, so default falls back to module.exports (#10360)',
  );
}

main();
