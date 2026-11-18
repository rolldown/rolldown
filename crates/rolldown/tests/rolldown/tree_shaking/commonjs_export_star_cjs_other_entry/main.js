import './barrel.js';

// Do not import `node:assert`: its `__toESM` call from `main.js` hits #11027
if (JSON.stringify(globalThis.evaluated) !== '["dep"]') {
  throw new Error(`unexpected evaluation: ${JSON.stringify(globalThis.evaluated)}`);
}
