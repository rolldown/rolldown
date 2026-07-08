import assert from 'node:assert';
import fs from 'node:fs';
import path from 'node:path';

const distDir = path.join(import.meta.dirname, 'dist');
const routeBFile = fs
  .readdirSync(distDir)
  .find((file) => file.startsWith('route-b') && file.endsWith('.js'));
assert.ok(routeBFile, 'route-b chunk should be emitted');

const routeBSource = fs.readFileSync(path.join(distDir, routeBFile), 'utf8');
assert.ok(
  !routeBSource.includes('require_feature'),
  'route-b must not contain route-a transferred CJS init',
);

const [routeA, routeB] = await Promise.all([
  import('./dist/route-a.js'),
  import('./dist/route-b.js'),
]);
assert.strictEqual(routeA.value, 'feature:leaf');
assert.strictEqual(routeB.value, 'leaf');
