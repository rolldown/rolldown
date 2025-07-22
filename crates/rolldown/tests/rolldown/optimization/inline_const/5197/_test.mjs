import assert from "node:assert";
import { glob, a, b } from "./dist/main.js";

(async () => {
	assert.strictEqual((await glob["./bar.css"]()).default, "bar");
	assert.strictEqual(a, "a");
	assert.strictEqual(b, "b");
})();
