import assert from 'node:assert';
import { spawnSync } from 'node:child_process';

const mainUrl = new URL('./dist/main.js', import.meta.url).href;
const namespaceEntry = spawnSync(
  process.execPath,
  [
    '--input-type=module',
    '--eval',
    `globalThis.__events = [];
     const entry = await import(${JSON.stringify(mainUrl)});
     const ns = await entry.loadNamespace();
     console.log(JSON.stringify({
       events: globalThis.__events,
       cn: ns.cn('a', 'b'),
       cloned: ns.cloneDeep({ value: 1 }),
       stack: new ns.Stack().items,
     }));`,
  ],
  { encoding: 'utf8' },
);

const namespaceEvents =
  globalThis.__configName === 'no-treeshake'
    ? ['cn', 'stack', 'clone-deep', 'main:a b']
    : ['cn', 'main:a b', 'stack', 'clone-deep'];
assert.deepStrictEqual(
  { status: namespaceEntry.status, stderr: namespaceEntry.stderr, stdout: namespaceEntry.stdout },
  {
    status: 0,
    stderr: '',
    stdout: `${JSON.stringify({ events: namespaceEvents, cn: 'a b', cloned: { value: 1 }, stack: [] })}\n`,
  },
);

globalThis.__events = [];

const entry = await import('./dist/main.js');

const initialEvents =
  globalThis.__configName === 'no-treeshake'
    ? ['cn', 'stack', 'clone-deep', 'main:a b']
    : ['cn', 'main:a b'];

assert.deepStrictEqual(globalThis.__events, initialEvents);

const route = await entry.loadRoute();

assert.strictEqual(route.value, 1);
assert.deepStrictEqual(
  globalThis.__events,
  globalThis.__configName === 'no-treeshake'
    ? [...initialEvents, 'route']
    : [...initialEvents, 'stack', 'clone-deep', 'route'],
);
