import type { InputOptions, OutputOptions } from 'rolldown';
import type { Modules } from './harness';

/**
 * A seeded generator of module graphs and option sets for the differential test of
 * `experimentalInlineCommonChunks`. Every graph has 2-4 entries and 3-10 shared modules with
 * random static import edges (mostly forward, sometimes backward, so cycles occur), entries that
 * import other entries, exports of every kind (`let`, `const`, function, class, default, an
 * export named `__proto__`), `export *` and named re-exports of other shared modules, namespace
 * imports, reassigned exports read live, CommonJS members in both `module.exports` and
 * `exports.x` style, default imports of CommonJS modules that are forwarded as
 * `export { default as x }` and whose properties are assigned to, route modules that only
 * re-export a CommonJS default and are loaded by dynamic import, external modules that log when
 * they evaluate (imported for their side effects or star re-exported), dynamic imports of shared
 * modules, side-effect logs, `this` checks through plain, optional, parenthesized-optional,
 * computed, `call`, `Reflect.apply` and tagged-template calls (the callee sometimes carrying the
 * `__NO_SIDE_EFFECTS__` annotation), globals read by shared modules, and local names, at the top
 * level and in nested scopes, that collide with each other and with the names the feature and the
 * bundler print (`share_<name>`, `exports`, `init_<name>`, `<name>_exports`, runtime helpers).
 * The option set varies internal export minification, tree shaking, `keepNames`, module order,
 * debug info, minification and a manual code-splitting group.
 */
export interface GeneratedGraph {
  input: Record<string, string>;
  modules: Modules;
  /** External modules, written next to the output; each logs when it evaluates. */
  files: Record<string, string>;
  inputOptions: InputOptions;
  output: OutputOptions;
  /** Minified output cannot be parsed by the registration-order check. */
  minified: boolean;
}

/** mulberry32: a small deterministic PRNG, so a failing seed is reproducible. */
function mulberry32(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const PLAIN_COLLIDING_NAMES = [
  'count',
  'value',
  'helper',
  'state',
  'init',
  'exports',
  'exports$1',
  'runtime',
  '__proto__',
  '__toESM',
  '__esmMin',
  'share_shared',
];

export function generateGraph(seed: number): GeneratedGraph {
  const rand = mulberry32(seed);
  const pick = <T>(items: T[]): T => items[Math.floor(rand() * items.length)];
  const chance = (p: number) => rand() < p;
  const int = (lo: number, hi: number) => lo + Math.floor(rand() * (hi - lo + 1));

  const entryCount = int(2, 4);
  const sharedCount = int(3, 10);
  const shared = Array.from({ length: sharedCount }, (_, i) => `s${i}`);
  const entries = Array.from({ length: entryCount }, (_, i) => `e${i}`);
  // The annotation marks calls of `f` pure; `f` still runs with `this` undefined.
  const pure = () => (chance(0.3) ? '/*#__NO_SIDE_EFFECTS__*/ ' : '');
  const cjs = new Set(shared.filter(() => chance(0.2)));
  // A module that imports an external stays a file; its position in the import order, and so
  // the external's evaluation order, must survive the projection of the records around it.
  const importsExternal = new Set(shared.filter(() => chance(0.2)));
  const modules: Modules = {};
  const input: Record<string, string> = {};
  const files: Record<string, string> = {};

  // Names the bundler or the feature may print at a file's root or inside a factory.
  const collidingName = (): string =>
    pick([
      pick(PLAIN_COLLIDING_NAMES),
      `share_${pick(shared)}`,
      `share_${pick(shared)}$1`,
      `init_${pick(shared)}`,
      `${pick(shared)}_exports`,
      `require_${pick(shared)}`,
    ]);

  const edges = new Map(shared.map((s) => [s, [] as string[]]));
  for (let i = 0; i < sharedCount; i++) {
    for (let j = 0; j < sharedCount; j++) {
      if (i === j) continue;
      if (j > i ? chance(0.35) : chance(0.06)) edges.get(shared[i])!.push(shared[j]);
    }
  }
  const entryImports = new Map(entries.map((e) => [e, new Set<string>()]));
  for (const s of shared) {
    const importers = entries.filter(() => chance(0.5));
    if (importers.length === 0) importers.push(pick(entries));
    for (const e of importers) entryImports.get(e)!.add(s);
  }
  for (const e of entries) {
    if (entryImports.get(e)!.size === 0) entryImports.get(e)!.add(pick(shared));
  }
  const dynamicTargets = new Set(shared.filter(() => chance(0.15)));
  // Route modules: each only re-exports a CommonJS module's default and is reached by a dynamic
  // import, so it is a dynamic entry whose exported binding is declared in the CommonJS chunk.
  const routes = [...cjs].filter(() => chance(0.5));
  for (const c of routes) {
    modules[`./route_${c}.js`] =
      `export { default } from './${c}.js';\nexport const via = 'route_${c}';`;
  }
  // ESM module -> the CommonJS dependencies whose default it forwards as `<d>_lib`.
  const forwardedCjs = new Map(shared.map((s) => [s, [] as string[]]));
  // Writes to a property of an imported CommonJS default export, in the assignment forms the
  // finalizer rewrites to `ns.default.prop`.
  const memberAssignment = (binding: string, tag: string): string =>
    pick([
      `${binding}.count_${tag} = (${binding}.count_${tag} ?? 0) + 1; ${binding}['k_${tag}'] = '${tag}';`,
      `${binding}.count_${tag} = 1; ${binding}.count_${tag} += 1; ${binding}['k_${tag}'] = '${tag}';`,
      `${binding}.count_${tag} = 0; ${binding}.count_${tag}++; ${binding}['k_${tag}'] = '${tag}';`,
    ]);

  // Calls that must see `this === undefined` through a namespace or bridge.
  const namespaceCall = (ns: string, f: string): string =>
    pick([
      `${ns}.${f}()`,
      `${ns}?.${f}()`,
      `(${ns}?.${f})()`,
      `(${ns}?.${f})?.()`,
      `${ns}['${f}']()`,
      `${ns}.${f}?.()`,
      `(0, ${ns}.${f})()`,
      `${ns}.${f}.call(undefined)`,
      `Reflect.apply(${ns}.${f}, undefined, [])`,
      `${ns}.${f}\`x\``,
      `(${ns}?.${f})\`x\``,
    ]);
  const namedCall = (f: string): string =>
    pick([`${f}()`, `${f}?.()`, `(0, ${f})()`, `[${f}][0]()`, `${f}\`x\``, `${f}.call(undefined)`]);
  // A nested scope that declares a colliding name and reads it, so renaming inside nested scopes
  // is exercised too.
  const nestedScope = (tag: string): string => {
    const local = collidingName();
    return `function nested_${tag}() { let ${local} = '${tag}'; return ${local}; }`;
  };

  // Inside a CommonJS module, `exports`, `module` and `require` are the wrapper's own parameters;
  // a local of that name would change what the module exports rather than test a collision.
  const cjsLocalName = (): string => {
    let name = collidingName();
    while (name === 'exports' || name === 'module' || name === 'require') name = collidingName();
    return name;
  };

  for (const s of shared) {
    const deps = edges.get(s)!;
    const local = cjs.has(s) ? cjsLocalName() : collidingName();
    const lines: string[] = [];
    if (importsExternal.has(s)) {
      files[`ext-${s}.js`] = `globalThis.__log('ext-${s}');\nexport const ext_${s} = '${s}';\n`;
      if (cjs.has(s)) lines.push(`require('#ext/${s}');`);
      // A star re-export of an external makes the finalizer print an import declaration inside
      // the module; such a module stays a file.
      else if (chance(0.4)) lines.push(`export * from '#ext/${s}';`);
      else lines.push(`import '#ext/${s}';`);
    }
    if (cjs.has(s)) {
      // A top-level local of a CommonJS module lives inside the `__commonJS` closure and must not
      // shadow a bridge or helper the module reads.
      let shadow = cjsLocalName();
      while (shadow === local) shadow = cjsLocalName();
      for (const d of deps) lines.push(`const ${d}_ns = require('./${d}.js');`);
      lines.push(`var ${shadow} = 'local';`);
      lines.push(`globalThis.__log('${s} body', ${JSON.stringify(local)}, ${shadow});`);
      lines.push(`let ${local} = 0;`);
      lines.push(
        `${pure()}function f() { ${local} += 1; return (this === undefined ? 'U' : 'B') + ${local}; }`,
      );
      lines.push(`const o = { tag: '${s}' };`);
      lines.push(`class C { constructor() { this.k = '${s}'; } }`);
      lines.push(nestedScope(s));
      for (const d of deps) {
        lines.push(
          `globalThis.__log('${s} sees ${d}', ${d}_ns.n, __id(${d}_ns.o), ${namespaceCall(`${d}_ns`, 'f')}, typeof ${d}_ns.default);`,
        );
      }
      if (chance(0.5)) {
        lines.push(
          `module.exports = { get n() { return ${local}; }, f, o, C, default: '${s}-default', nested: nested_${s}() };`,
        );
      } else {
        lines.push(
          `Object.defineProperty(exports, 'n', { enumerable: true, get() { return ${local}; } });`,
        );
        lines.push(
          `exports.f = f; exports.o = o; exports.C = C; exports.default = '${s}-default';`,
        );
        lines.push(`exports.nested = nested_${s}();`);
      }
    } else {
      const viaNs = new Map(deps.map((d) => [d, chance(0.3)]));
      // A CommonJS dependency's default export (its `module.exports`) imported as a binding: reads
      // go through `ns.default`, and an assignment to one of its properties is rewritten to
      // `ns.default.prop = v`, where `ns` must be the bridge when a record owns it.
      const cjsDefault = deps.filter((d) => cjs.has(d) && chance(0.4));
      for (const d of deps) {
        if (viaNs.get(d)) lines.push(`import * as ${d}_ns from './${d}.js';`);
        else {
          lines.push(
            `import { n as ${d}_n, f as ${d}_f, o as ${d}_o, C as ${d}_C } from './${d}.js';`,
          );
        }
        if (cjsDefault.includes(d)) lines.push(`import ${d}_def from './${d}.js';`);
      }
      // Forwarding a CommonJS default export gives the reader a binding whose owner is this
      // module's chunk while the object comes from the CommonJS module.
      for (const d of cjsDefault) {
        if (chance(0.5)) {
          lines.push(`export { default as ${d}_lib } from './${d}.js';`);
          forwardedCjs.get(s)!.push(d);
        }
      }
      // Re-exports: a star re-export forwards everything but the module's own names, a named
      // re-export forwards one binding under a new name.
      const reexported = deps.filter((d) => !cjs.has(d) && chance(0.25));
      for (const d of reexported) {
        if (chance(0.5)) lines.push(`export * from './${d}.js';`);
        else lines.push(`export { n as ${d}_again, f as ${d}_f_again } from './${d}.js';`);
      }
      lines.push(`globalThis.__log('${s} body', ${JSON.stringify(local)});`);
      lines.push(`export let n = 0;`);
      lines.push(`let ${local} = 10;`);
      lines.push(
        `${pure()}export function f() { n += 1; ${local} += 1; return (this === undefined ? 'U' : 'B') + n + ':' + ${local}; }`,
      );
      lines.push(`export const o = { tag: '${s}' };`);
      lines.push(`export class C { constructor() { this.k = '${s}'; } }`);
      lines.push(nestedScope(s));
      lines.push(`export const nested = nested_${s}();`);
      if (local !== '__proto__' && chance(0.15)) {
        lines.push(`export let __proto__ = String(globalThis.__p ?? 'P') + n;`);
      }
      if (chance(0.5))
        lines.push(`export default function ${s}Default() { return '${s}-default'; }`);
      else lines.push(`export default '${s}-default';`);
      if (chance(0.3)) {
        lines.push(
          `export const g = typeof globalThis.__marker === 'undefined' ? 'no-marker' : globalThis.__marker;`,
        );
      }
      const named = deps.filter((d) => !viaNs.get(d));
      if (named.length > 0 && chance(0.3))
        lines.push(`export { ${named[0]}_n as ${named[0]}_alias };`);
      for (const d of deps) {
        if (viaNs.get(d)) {
          lines.push(
            `globalThis.__log('${s} sees ${d}', ${d}_ns.n, __id(${d}_ns.o), ${namespaceCall(`${d}_ns`, 'f')}, ${namespaceCall(`${d}_ns`, 'f')}, __id(${d}_ns));`,
          );
        } else {
          lines.push(
            `globalThis.__log('${s} sees ${d}', ${d}_n, __id(${d}_o), ${namedCall(`${d}_f`)}, ${namedCall(`${d}_f`)}, new ${d}_C().k);`,
          );
        }
        if (cjsDefault.includes(d)) {
          lines.push(memberAssignment(`${d}_def`, `${s}`));
          lines.push(
            `globalThis.__log('${s} writes ${d}', ${d}_def.count_${s}, ${d}_def['k_${s}'], __id(${d}_def.o));`,
          );
        }
      }
      if (chance(0.4)) lines.push(`n = ${int(1, 9)};`);
    }
    modules[`./${s}.js`] = lines.join('\n');
  }

  for (const [index, e] of entries.entries()) {
    const imports = [...entryImports.get(e)!];
    const lines: string[] = [];
    const local = collidingName();
    const viaNs = new Map(imports.map((s) => [s, cjs.has(s) || chance(0.3)]));
    const writesTo: string[] = [];
    for (const s of imports) {
      if (viaNs.get(s)) lines.push(`import * as ${s}_ns from './${s}.js';`);
      else {
        lines.push(
          `import def_${s}, { n as ${s}_n, f as ${s}_f, o as ${s}_o, C as ${s}_C } from './${s}.js';`,
        );
      }
      if (cjs.has(s) && chance(0.4)) {
        lines.push(`import ${s}_def from './${s}.js';`);
        writesTo.push(`${s}_def`);
      }
      for (const d of forwardedCjs.get(s)!) {
        if (chance(0.5)) {
          lines.push(`import { ${d}_lib as ${d}_lib_via_${s} } from './${s}.js';`);
          writesTo.push(`${d}_lib_via_${s}`);
        }
      }
    }
    // An entry that imports an earlier entry: that entry's chunk hosts an entry module and stays
    // a file.
    const importedEntry = index > 0 && chance(0.2) ? entries[int(0, index - 1)] : undefined;
    if (importedEntry) lines.push(`import * as ${importedEntry}_ns from './${importedEntry}.js';`);
    lines.push(`let ${local} = 'entry-${e}';`);
    lines.push(nestedScope(e));
    lines.push(`globalThis.__log('${e} body', ${local}, nested_${e}());`);
    if (importedEntry) {
      lines.push(
        `globalThis.__log('${e} sees ${importedEntry}', typeof ${importedEntry}_ns.result);`,
      );
    }
    for (const s of imports) {
      if (viaNs.get(s)) {
        lines.push(
          `globalThis.__log('${e} reads ${s}', ${s}_ns.n, __id(${s}_ns.o), ${namespaceCall(`${s}_ns`, 'f')}, ${namespaceCall(`${s}_ns`, 'f')}, __id(${s}_ns), typeof ${s}_ns.default, ${s}_ns.nested);`,
        );
      } else {
        lines.push(
          `globalThis.__log('${e} reads ${s}', ${s}_n, __id(${s}_o), ${namedCall(`${s}_f`)}, ${namedCall(`${s}_f`)}, new ${s}_C().k, typeof def_${s});`,
        );
        lines.push(`globalThis.__log('${e} reads ${s} again', ${s}_n);`);
      }
    }
    // Dynamic imports run one after another on one chain shared by every entry: two in flight at
    // once resolve in an order that depends on how many files each has to load, which the two
    // builds legitimately differ in, and an entry that imports another entry starts that entry's
    // dynamic imports as well as its own.
    const dynamic: string[] = [];
    for (const s of imports) {
      if (dynamicTargets.has(s)) {
        dynamic.push(
          `import('./${s}.js').then((ns) => globalThis.__log('${e} dyn ${s}', ns.n, __id(ns.o)))`,
        );
      }
      if (routes.includes(s) && chance(0.6)) {
        dynamic.push(
          `import('./route_${s}.js').then((ns) => globalThis.__log('${e} route ${s}', typeof ns.default, ns.via, __id(ns.default)))`,
        );
      }
    }
    if (dynamic.length > 0) {
      lines.push(`globalThis.__chain ??= Promise.resolve();`);
      for (const step of dynamic)
        lines.push(`globalThis.__chain = globalThis.__chain.then(() => ${step});`);
    }
    for (const binding of writesTo) {
      lines.push(memberAssignment(binding, e));
      lines.push(
        `globalThis.__log('${e} writes ${binding}', ${binding}.count_${e}, ${binding}['k_${e}'], ${binding}.n, __id(${binding}.o));`,
      );
    }
    if (chance(0.5)) lines.push(`export const result = 'ok-${e}';`);
    modules[`./${e}.js`] = lines.join('\n');
    input[e] = `./${e}.js`;
  }

  const inputOptions: InputOptions = {};
  const output: OutputOptions = {};
  if (importsExternal.size > 0) inputOptions.external = (id) => id.startsWith('#ext/');
  if (chance(0.3)) output.minifyInternalExports = false;
  if (chance(0.15)) inputOptions.treeshake = false;
  if (chance(0.15)) output.keepNames = true;
  if (chance(0.15)) inputOptions.experimental = { chunkModulesOrder: 'module-id' };
  if (chance(0.1))
    inputOptions.experimental = { ...inputOptions.experimental, attachDebugInfo: 'none' };
  if (chance(0.15)) {
    // A manual group is never a record; records and files around it must still agree.
    output.codeSplitting = { groups: [{ name: 'grp', test: new RegExp(`${pick(shared)}\\.js$`) }] };
  }
  const minified = chance(0.15);
  if (minified) output.minify = true;

  return { input, modules, files, inputOptions, output, minified };
}
