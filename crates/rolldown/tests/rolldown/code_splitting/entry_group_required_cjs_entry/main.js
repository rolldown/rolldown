const leaf = require("./leaf.js");
if (leaf.ok !== true) {
  throw new Error(`leaf not initialized: ${JSON.stringify(leaf)}`);
}
module.exports = { ok: leaf.ok };
