/** Seeded graphs with live bindings, shared objects, CommonJS, and nested dynamic imports. */
export function generateDynamicGraph(seed: number) {
  let state = seed;
  const pick = (bound: number) => {
    state = (state + 0x6d2b79f5) >>> 0;
    let value = state;
    value = Math.imul(value ^ (value >>> 15), value | 1);
    value ^= value + Math.imul(value ^ (value >>> 7), value | 61);
    return Math.floor((((value ^ (value >>> 14)) >>> 0) / 4294967296) * bound);
  };
  const sharedCount = 4 + pick(6);
  const routeCount = 3 + pick(4);
  const entryCount = 2 + pick(2);
  const modules: Record<string, string> = {};
  const input: Record<string, string> = {};
  for (let i = 0; i < sharedCount; i++) {
    // Vary acyclic dependencies and small source cycles without putting every shared module in
    // one SCC. All imported values are read after initialization.
    const dependency =
      seed % 3 === 0
        ? i > 0
          ? pick(i)
          : undefined
        : seed % 3 === 1
          ? (i ^ 1) < sharedCount
            ? i ^ 1
            : undefined
          : pick(sharedCount);
    modules[`./s${i}.js`] = `
      ${
        dependency !== undefined && dependency !== i
          ? `import { value as other } from './s${dependency}.js';`
          : 'const other = -1;'
      }
      globalThis.__log('init', 's${i}');
      export let value = ${i};
      export const object = { module: ${i} };
      export function bump() { value++; }
      export function read() { return [value, other]; }
      export function receiver() { return Object.keys(this).sort().join(','); }
    `;
    modules[`./b${i}.js`] = `export * from './s${i}.js';`;
  }
  for (let i = 0; i < 2; i++) {
    modules[`./c${i}.cjs`] = `
      globalThis.__log('init', 'c${i}');
      const other = require('./c${1 - i}.cjs');
      exports.object = { cjs: ${i} };
      exports.read = () => [exports.object.cjs, other.object.cjs];
    `;
  }
  for (let i = 0; i < routeCount; i++) {
    const shared = pick(sharedCount);
    const next = (i + 1) % routeCount;
    modules[`./r${i}.js`] = `
      import * as shared from './b${shared}.js';
      import cjs from './c${pick(2)}.cjs';
      export { value, object, bump, read, receiver } from './b${shared}.js';
      globalThis.__log('init', 'r${i}');
      export async function visit() {
        const next = await import('./r${next}.js');
        const again = await import('./r${next}.js');
        globalThis.__log('nested', 'r${i}', next === again, __id(next.object), next.read());
        next.bump();
        globalThis.__log('live', next.value, shared.read(), cjs.read(), __id(cjs.object));
      }
    `;
  }
  for (let i = 0; i < entryCount; i++) {
    const shared = pick(sharedCount);
    const routes = Array.from({ length: routeCount }, (_, route) => route);
    for (let j = routes.length - 1; j > 0; j--) {
      const other = pick(j + 1);
      [routes[j], routes[other]] = [routes[other], routes[j]];
    }
    const loads = routes.map((route, index) => {
      // Alternate complete namespace use and statically known export use.
      return index % 2 === 0
        ? `globalThis.__chain = globalThis.__chain.then(async () => {
            const ns = await import('./r${route}.js');
            globalThis.__log('namespace', 'e${i}', 'r${route}', Object.keys(ns), ns.receiver(), __id(ns.object));
            await ns.visit();
            globalThis.__log('value', ns.value);
          });`
        : `globalThis.__chain = globalThis.__chain.then(async () => {
            const { read, bump, visit } = await import('./r${route}.js');
            bump();
            globalThis.__log('partial', 'e${i}', 'r${route}', read());
            await visit();
          });`;
    });
    modules[`./e${i}.js`] = `
      import { value, object, read } from './b${shared}.js';
      ${i > 0 && seed % 3 === 0 ? `import './e${i - 1}.js';` : ''}
      export { value, object, read };
      globalThis.__log('entry', 'e${i}', value, __id(object), read());
      globalThis.__chain ??= Promise.resolve();
      ${loads.join('\n')}
    `;
    input[`e${i}`] = `./e${i}.js`;
  }
  return { input, modules };
}
