import nodeAssert from 'node:assert';

async function main() {
  const exports = await import('./lib.js');

  nodeAssert.deepEqual(Object.keys(exports).sort(), ['parse']);
  nodeAssert.strictEqual(exports.parse, 'parse', 'Expected export exists and is correct');
  nodeAssert.strictEqual(exports.default, undefined, 'Target has __esModule, so no auto-generated default export');
}

main()
