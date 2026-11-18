// A Worker's main module runs once per isolate, at startup; its handlers (`fetch`, `scheduled`,
// `queue`) run once per event. An import that only code inside a function reads can load in
// that function: a dynamic import there takes the import's closure out of startup. In any
// module, a function that is already `async` returns a promise, so an `await import()` in it
// changes no caller. This module finds those imports in the source and the graph edge of each.
// See AGENTS.md, "If the target is a Cloudflare Worker".

import fs from 'node:fs';
import { createRequire } from 'node:module';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

import { isVirtual } from './startup-cpu.mjs';

/** rolldown's `parseAst`: from the rolldown that the app's vite uses, else any reachable one. */
export async function loadParseAst(appDir) {
  const requires = [];
  try {
    const app = createRequire(path.join(appDir, 'package.json'));
    try {
      requires.push(createRequire(fs.realpathSync(app.resolve('vite/package.json'))));
    } catch {}
    requires.push(app);
  } catch {}
  requires.push(createRequire(import.meta.url));
  for (const req of requires) {
    try {
      const mod = await import(pathToFileURL(req.resolve('rolldown/parseAst')).href);
      if (typeof mod.parseAst === 'function') return mod.parseAst;
    } catch {}
  }
  return null;
}

const LANG = { '.ts': 'ts', '.mts': 'ts', '.cts': 'ts', '.tsx': 'tsx', '.jsx': 'jsx' };

/** The ESTree program of a source file, or null when it does not parse. */
export function parseFile(parseAst, file, text) {
  try {
    return parseAst(text, { lang: LANG[path.extname(file)] ?? 'js', sourceType: 'module' }, file);
  } catch {
    return null;
  }
}

// TypeScript nodes that hold a value. Every other TypeScript node is a type: it reads no binding.
const TS_VALUE = new Set([
  'TSAsExpression',
  'TSSatisfiesExpression',
  'TSNonNullExpression',
  'TSTypeAssertion',
  'TSInstantiationExpression',
  'TSExportAssignment',
]);
const TYPE_KEYS = new Set([
  'typeAnnotation',
  'returnType',
  'typeParameters',
  'typeArguments',
  'superTypeArguments',
  'superTypeParameters',
  'implements',
]);
const FUNCTIONS = new Set(['FunctionDeclaration', 'FunctionExpression', 'ArrowFunctionExpression']);

const unwrap = (node) => {
  while (node && (node.type === 'ParenthesizedExpression' || TS_VALUE.has(node.type))) {
    node = node.expression;
  }
  return node;
};

const keyName = (key) =>
  key?.type === 'Identifier'
    ? key.name
    : key?.type === 'PrivateIdentifier'
      ? `#${key.name}`
      : key?.type === 'Literal'
        ? String(key.value)
        : null;

/**
 * Where a module reads each value import. Returns one row per import declaration:
 * `{ source, sideEffect, names: [{ name, reads, startup, fns, asyncOnly }] }`. A read is at
 * startup when module-scope code makes it, or a module-scope function or class that
 * module-scope code calls (directly or through other such functions). Every other read sits in
 * a function that runs later, such as a handler; `fns` names those functions. `asyncOnly`: every
 * read sits inside an `async` function (at any depth), none at startup.
 */
export function importUses(program) {
  const rows = [];
  const byName = new Map();
  for (const node of program.body) {
    if (node.type !== 'ImportDeclaration' || node.importKind === 'type') continue;
    const specs = node.specifiers.filter((s) => s.importKind !== 'type');
    if (node.specifiers.length && !specs.length) continue;
    const row = {
      source: node.source.value,
      sideEffect: !node.specifiers.length,
      names: specs.map((s) => ({
        name: s.local.name,
        reads: 0,
        startup: false,
        fns: new Set(),
        syncReads: 0,
      })),
    };
    for (const n of row.names) byName.set(n.name, n);
    rows.push(row);
  }

  // ctx.fn: the label of the innermost function around the node, null at module scope (an IIFE
  // and the static parts of a module-scope class run there too). ctx.owner: the module-scope
  // function, class or variable around the node. ctx.label: the name a parent gives a value.
  // ctx.async: an `async` function holds the node.
  const reads = [];
  const calledAtTop = new Set();
  const ownerReads = new Map();
  const read = (name, ctx) => {
    if (ctx.owner) {
      if (!ownerReads.has(ctx.owner)) ownerReads.set(ctx.owner, new Set());
      ownerReads.get(ctx.owner).add(name);
    }
    if (byName.has(name)) reads.push({ name, fn: ctx.fn, owner: ctx.owner, async: ctx.async });
  };

  const pattern = (p, ctx) => {
    if (!p) return;
    switch (p.type) {
      case 'Identifier':
        return;
      case 'AssignmentPattern':
        pattern(p.left, ctx);
        walk(p.right, ctx);
        return;
      case 'ArrayPattern':
        for (const e of p.elements) pattern(e, ctx);
        return;
      case 'ObjectPattern':
        for (const prop of p.properties) {
          if (prop.type === 'RestElement') pattern(prop.argument, ctx);
          else {
            if (prop.computed) walk(prop.key, ctx);
            pattern(prop.value, ctx);
          }
        }
        return;
      case 'RestElement':
        pattern(p.argument, ctx);
        return;
      case 'TSParameterProperty':
        pattern(p.parameter, ctx);
        return;
      default:
        walk(p, ctx);
    }
  };

  const fn = (node, ctx, runsNow) => {
    const label = node.id?.name ?? ctx.label ?? ctx.fn ?? '(function)';
    const async = ctx.async || Boolean(node.async);
    const inner = runsNow
      ? { ...ctx, async, label: null }
      : { ...ctx, fn: label, async, label: null };
    for (const p of node.params ?? []) pattern(p, inner);
    walk(node.body, inner);
  };

  const children = (node, ctx) => {
    const next = { ...ctx, label: null };
    for (const [key, value] of Object.entries(node)) {
      if (key === 'type' || TYPE_KEYS.has(key) || !value || typeof value !== 'object') continue;
      walk(value, next);
    }
  };

  const walk = (node, ctx) => {
    if (!node || typeof node !== 'object') return;
    if (Array.isArray(node)) {
      for (const n of node) walk(n, ctx);
      return;
    }
    const t = node.type;
    if (typeof t !== 'string') return;
    if (t.startsWith('TS')) {
      if (TS_VALUE.has(t)) walk(node.expression, ctx);
      return;
    }
    switch (t) {
      case 'Identifier':
        read(node.name, ctx);
        return;
      case 'ImportDeclaration':
      case 'ExportAllDeclaration':
      case 'MetaProperty':
      case 'PrivateIdentifier':
      case 'Literal':
      case 'TemplateElement':
        return;
      case 'LabeledStatement':
        walk(node.body, ctx);
        return;
      case 'BreakStatement':
      case 'ContinueStatement':
      case 'JSXClosingElement':
        return;
      case 'JSXOpeningElement': {
        // `<Card>` and `<ui.Card>` read a binding; a lowercase tag is an intrinsic element.
        let tag = node.name;
        while (tag?.type === 'JSXMemberExpression') tag = tag.object;
        if (tag?.type === 'JSXIdentifier' && (tag !== node.name || !/^[a-z]/.test(tag.name))) {
          read(tag.name, ctx);
        }
        for (const a of node.attributes ?? []) {
          walk(a.type === 'JSXSpreadAttribute' ? a.argument : a.value, ctx);
        }
        return;
      }
      case 'ExportNamedDeclaration':
        if (node.source || node.exportKind === 'type') return;
        walk(node.declaration, ctx);
        for (const s of node.specifiers ?? []) if (s.exportKind !== 'type') walk(s.local, ctx);
        return;
      case 'ExportDefaultDeclaration': {
        const d = node.declaration;
        const owner = ctx.owner ?? d?.id?.name ?? 'default';
        walk(d, { ...ctx, owner, label: d?.id?.name ?? 'default' });
        return;
      }
      case 'VariableDeclaration':
        for (const d of node.declarations) walk(d, ctx);
        return;
      case 'VariableDeclarator': {
        const name = node.id.type === 'Identifier' ? node.id.name : null;
        pattern(node.id, ctx);
        const owner = ctx.fn == null && ctx.owner == null ? name : ctx.owner;
        walk(node.init, { ...ctx, owner, label: name });
        return;
      }
      case 'FunctionDeclaration': {
        const owner = ctx.fn == null && ctx.owner == null ? (node.id?.name ?? null) : ctx.owner;
        fn(node, { ...ctx, owner }, false);
        return;
      }
      case 'FunctionExpression':
      case 'ArrowFunctionExpression':
        fn(node, ctx, false);
        return;
      case 'ClassDeclaration':
      case 'ClassExpression': {
        const name = node.id?.name ?? ctx.label;
        const owner = ctx.fn == null && ctx.owner == null ? (name ?? null) : ctx.owner;
        for (const d of node.decorators ?? []) walk(d, ctx);
        walk(node.superClass, ctx);
        walk(node.body, { ...ctx, owner, label: null });
        return;
      }
      case 'MethodDefinition':
      case 'PropertyDefinition':
      case 'AccessorProperty': {
        if (node.computed) walk(node.key, ctx);
        for (const d of node.decorators ?? []) walk(d, ctx);
        const label = keyName(node.key) ?? '(member)';
        if (t === 'MethodDefinition') fn(node.value, { ...ctx, label }, false);
        // A static field runs when the class is defined; an instance field, per instance.
        else if (node.static) walk(node.value, { ...ctx, label: null });
        else walk(node.value, { ...ctx, fn: label, label: null });
        return;
      }
      case 'StaticBlock':
        walk(node.body, ctx);
        return;
      case 'Property':
        if (node.computed) walk(node.key, ctx);
        walk(node.value, { ...ctx, label: keyName(node.key) });
        return;
      case 'MemberExpression':
        walk(node.object, { ...ctx, label: null });
        if (node.computed) walk(node.property, { ...ctx, label: null });
        return;
      case 'AssignmentExpression': {
        const left = node.left;
        const name =
          left.type === 'Identifier'
            ? left.name
            : left.type === 'MemberExpression' && !left.computed
              ? keyName(left.property)
              : null;
        if (left.type !== 'Identifier') pattern(left, { ...ctx, label: null });
        walk(node.right, { ...ctx, label: name });
        return;
      }
      case 'CallExpression':
      case 'NewExpression': {
        const callee = unwrap(node.callee);
        // An IIFE runs where it stands.
        if (callee && FUNCTIONS.has(callee.type)) fn(callee, ctx, true);
        else {
          if (ctx.fn == null && callee?.type === 'Identifier') calledAtTop.add(callee.name);
          walk(node.callee, { ...ctx, label: null });
        }
        walk(node.arguments, { ...ctx, label: null });
        return;
      }
      case 'TaggedTemplateExpression': {
        const tag = unwrap(node.tag);
        if (ctx.fn == null && tag?.type === 'Identifier') calledAtTop.add(tag.name);
        children(node, ctx);
        return;
      }
      case 'CatchClause':
        pattern(node.param, ctx);
        walk(node.body, ctx);
        return;
      case 'ForInStatement':
      case 'ForOfStatement':
        if (node.left.type === 'VariableDeclaration') walk(node.left, ctx);
        else pattern(node.left, ctx);
        walk(node.right, ctx);
        walk(node.body, ctx);
        return;
      default:
        children(node, ctx);
    }
  };

  walk(program.body, { fn: null, owner: null, label: null, async: false });

  // Module-scope functions and classes that run at startup: called at module scope, or read
  // by one that runs.
  const atStartup = new Set();
  const queue = [...calledAtTop];
  while (queue.length) {
    const name = queue.pop();
    if (atStartup.has(name)) continue;
    atStartup.add(name);
    for (const next of ownerReads.get(name) ?? []) if (!atStartup.has(next)) queue.push(next);
  }
  for (const r of reads) {
    const n = byName.get(r.name);
    n.reads += 1;
    if (r.fn == null || (r.owner != null && atStartup.has(r.owner))) n.startup = true;
    else {
      n.fns.add(r.fn);
      if (!r.async) n.syncReads += 1;
    }
  }
  return rows.map((row) => ({
    ...row,
    names: row.names.map(({ syncReads, ...n }) => ({
      ...n,
      fns: [...n.fns],
      asyncOnly: n.reads > 0 && !n.startup && syncReads === 0,
    })),
  }));
}

/**
 * An import that can load inside the functions that read it: a value import that no startup
 * code reads, read at least once.
 */
export function readOnlyLater(row) {
  if (row.sideEffect) return false;
  const used = row.names.filter((n) => n.reads > 0);
  return used.length > 0 && used.every((n) => !n.startup);
}

/**
 * An import that can load inside the `async` functions that read it, with no caller changed:
 * a value import that only code inside `async` functions reads, read at least once.
 */
export function readOnlyInAsync(row) {
  if (row.sideEffect) return false;
  const used = row.names.filter((n) => n.reads > 0);
  return used.length > 0 && used.every((n) => n.asyncOnly);
}

/**
 * The app modules at startup worth reading for such imports: those with a static import whose
 * target retains `minBytes` or `minModules`. Taking one edge out of the graph frees only
 * modules that its target dominates, so the target's `retainedBytes` and
 * `retainedModuleCount` bound the gain. Returns module indices, largest bound first.
 */
export function asyncImportModules(graph, isApp, { skip = null, minBytes, minModules }) {
  const found = [];
  graph.modules.forEach((m, i) => {
    if (i === skip || !m.staticReachable || isVirtual(m.id) || !isApp(m.id)) return;
    let bound = -1;
    for (const [to, dynamic] of m.imports ?? []) {
      const t = graph.modules[to];
      if (dynamic || isVirtual(t.id)) continue;
      const bytes = t.retainedBytes ?? 0;
      if (bytes >= minBytes || (t.retainedModuleCount ?? 0) >= minModules) {
        bound = Math.max(bound, bytes);
      }
    }
    if (bound >= 0) found.push([i, bound]);
  });
  return found.sort((a, b) => b[1] - a[1] || a[0] - b[0]).map(([i]) => i);
}

/**
 * Text, data and wasm modules (`__CLOUDFLARE_MODULE__<kind>__<path>__CLOUDFLARE_MODULE__`) count 0 bytes in the
 * graph, but the isolate loads each file. Returns their count and size on disk.
 */
export function textModuleBytes(modules) {
  let count = 0;
  let bytes = 0;
  for (const m of modules) {
    const file = /^__CLOUDFLARE_MODULE__\w+?__(.+?)(?:__CLOUDFLARE_MODULE__)?$/.exec(m.id)?.[1];
    if (!file) continue;
    count += 1;
    try {
      bytes += fs.statSync(file).size;
    } catch {}
  }
  return { count, bytes };
}

const stem = (p) =>
  p
    .replaceAll('\\', '/')
    .replace(/\.(?:[cm]?[jt]sx?|json)$/, '')
    .replace(/\/index$/, '');

const packageOf = (spec) =>
  spec.startsWith('@') ? spec.split('/').slice(0, 2).join('/') : spec.split('/')[0];

/** The `name` of the nearest package.json above a file, cached per directory. */
function nearestPackageName(file, cache) {
  let dir = path.dirname(file);
  const seen = [];
  for (;;) {
    if (cache.has(dir)) {
      const name = cache.get(dir);
      for (const d of seen) cache.set(d, name);
      return name;
    }
    seen.push(dir);
    let name;
    try {
      name = JSON.parse(fs.readFileSync(path.join(dir, 'package.json'), 'utf8')).name ?? null;
    } catch {}
    if (name !== undefined) {
      for (const d of seen) cache.set(d, name);
      return name;
    }
    const parent = path.dirname(dir);
    if (parent === dir) return null;
    dir = parent;
  }
}

/**
 * The graph module each import source of the main module names, among the main module's
 * static imports. The report has no specifiers, so a source is matched by its path (relative),
 * by its package (bare), or by its path after the alias segment (`@/lib/x`). A source and an
 * edge that are the only ones left belong together. Returns `Map<source, module index>`.
 */
export function matchImportTargets(graph, mainIndex, sources, rootDir) {
  const main = graph.modules[mainIndex];
  const abs = (id) => (path.isAbsolute(id) ? id : path.resolve(rootDir, id));
  const free = new Set(
    (main.imports ?? [])
      .filter(([to, dynamic]) => !dynamic && !isVirtual(graph.modules[to].id))
      .map(([to]) => to),
  );
  const found = new Map();
  const take = (source, index) => {
    found.set(source, index);
    free.delete(index);
  };
  const relative = (s) => /^\.\.?\//.test(s);
  for (const s of sources) {
    if (!relative(s)) continue;
    const want = stem(path.resolve(path.dirname(abs(main.id)), s));
    const hit = [...free].find((i) => stem(abs(graph.modules[i].id)) === want);
    if (hit != null) take(s, hit);
  }
  const names = new Map();
  for (const s of sources) {
    if (found.has(s) || relative(s)) continue;
    const pkg = packageOf(s);
    const sub = s.slice(pkg.length + 1);
    const same = [...free].filter((i) => {
      const id = graph.modules[i].id.replaceAll('\\', '/');
      if (id === s || id.startsWith(`${pkg}/`) || id.includes(`/node_modules/${pkg}/`)) return true;
      return path.isAbsolute(id) && nearestPackageName(id, names) === pkg;
    });
    const hit =
      same.length === 1
        ? same[0]
        : same.find((i) => sub && stem(graph.modules[i].id).endsWith(`/${stem(sub)}`));
    if (hit != null) take(s, hit);
  }
  for (const s of sources) {
    if (found.has(s)) continue;
    const rest = s.split('/').slice(1).join('/');
    if (!rest) continue;
    const hits = [...free].filter((i) => stem(abs(graph.modules[i].id)).endsWith(`/${stem(rest)}`));
    if (hits.length === 1) take(s, hits[0]);
  }
  const left = sources.filter((s) => !found.has(s));
  if (left.length === 1 && free.size === 1) take(left[0], [...free][0]);
  return found;
}

/**
 * The app's own main module: an app module the plugin's virtual entry imports (the largest
 * one when it imports several), or the entry itself when it is app code. `isApp` tells app
 * modules from packages.
 */
export function mainModuleIndex(graph, isApp) {
  const byId = new Map(graph.modules.map((m, i) => [m.id, i]));
  let best = null;
  const consider = (i) => {
    const m = graph.modules[i];
    if (isVirtual(m.id) || !isApp(m.id)) return;
    if (best == null || (m.retainedBytes ?? 0) > (graph.modules[best].retainedBytes ?? 0)) best = i;
  };
  for (const id of graph.entryModules ?? []) {
    const start = byId.get(id);
    if (start == null) continue;
    if (!isVirtual(graph.modules[start].id)) {
      consider(start);
      continue;
    }
    // Through the plugin's virtual modules to the first app modules.
    const seen = new Set([start]);
    const queue = [start];
    while (queue.length) {
      const i = queue.shift();
      for (const [to, dynamic] of graph.modules[i].imports ?? []) {
        if (dynamic || seen.has(to)) continue;
        seen.add(to);
        if (isVirtual(graph.modules[to].id)) queue.push(to);
        else consider(to);
      }
    }
  }
  return best;
}
