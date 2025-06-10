import assert from "node:assert";

(async () => {
	const mod = await import("./foo.js");
	assert.strictEqual(mod.default, "foo");
})();
