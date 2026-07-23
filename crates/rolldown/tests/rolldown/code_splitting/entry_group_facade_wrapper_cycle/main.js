const leaf = require('./leaf.js');
const leafEsm = require('./leaf-esm.js');

if (leaf.ok !== true) {
  throw new Error(`cjs leaf not initialized: ${JSON.stringify(leaf)}`);
}
if (leafEsm.ok !== true) {
  throw new Error(`esm leaf not initialized: ${JSON.stringify(leafEsm)}`);
}

module.exports = { ok: leaf.ok && leafEsm.ok };
