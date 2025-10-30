import mod from "./rule";
import assert from "node:assert";

const plugin = {
  mod,
};
assert.strictEqual(plugin.mod.meta, 19);
export default plugin;
