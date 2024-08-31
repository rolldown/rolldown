import assert from "node:assert";

import("./a").then((mod) => {
	assert.strictEqual(mod.a, 1000);
});

import("./b").then((mod) => {
	assert.strictEqual(mod.test, 1000);
});
