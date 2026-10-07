// Runs the built files given as arguments, in order, as roots of one fresh Node process and
// prints what happened as JSON: the ordered log, each root's exports, and any error, all in a
// form that can be compared between two builds. Test modules log through `globalThis.__log` and
// name object identities through `globalThis.__id`. Work a root starts without awaiting it (a
// dynamic import whose `.then` logs) goes on the promise chain `globalThis.__chain`; a root's
// exports are read once that chain has settled.
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const logs = [];
const ids = new Map();

globalThis.__id = (value) => {
  if (!ids.has(value)) {
    ids.set(value, `#${ids.size + 1}`);
  }
  return ids.get(value);
};

function describe(value) {
  if (value === undefined) return 'undefined';
  if (value === null) return 'null';
  if (typeof value === 'function') return `[function ${value.name}]`;
  if (typeof value === 'object')
    return `[object ${globalThis.__id(value)} ${JSON.stringify(value)}]`;
  return `${String(value)}:${typeof value}`;
}

function describeError(error) {
  if (error === undefined) return { kind: 'undefined' };
  if (error === null) return { kind: 'null' };
  if (error instanceof Error) return { kind: 'error', name: error.name, message: error.message };
  return { kind: 'value', value: describe(error) };
}

globalThis.__log = (...args) => {
  logs.push(args.map(describe).join(' '));
};

const result = { logs, exports: null, error: null, roots: [] };
for (const file of process.argv.slice(2)) {
  const root = { file: path.basename(file), exports: null, error: null };
  try {
    const namespace = await import(pathToFileURL(file).href);
    await globalThis.__chain;
    root.exports = Object.fromEntries(
      Object.keys(namespace)
        .sort()
        .map((key) => [key, describe(namespace[key])]),
    );
  } catch (error) {
    root.error = describeError(error);
  }
  result.roots.push(root);
  result.exports = root.exports;
  result.error ??= root.error;
}
process.stdout.write(JSON.stringify(result));
