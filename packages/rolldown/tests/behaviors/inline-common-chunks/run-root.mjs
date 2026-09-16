// Runs one built entry as the root of a fresh Node process and prints what happened as JSON:
// the ordered log, the entry's exports, and any error, all in a form that can be compared
// between two builds. Test modules log through `globalThis.__log` and name object identities
// through `globalThis.__id`.
import { pathToFileURL } from 'node:url';

const entry = process.argv[2];
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

const result = { logs, exports: null, error: null };
try {
  const namespace = await import(pathToFileURL(entry).href);
  // Let `.then` chains scheduled by the entry settle.
  await new Promise((resolve) => setTimeout(resolve, 50));
  result.exports = Object.fromEntries(
    Object.keys(namespace)
      .sort()
      .map((key) => [key, describe(namespace[key])]),
  );
} catch (error) {
  result.error = describeError(error);
}
process.stdout.write(JSON.stringify(result));
