import assert from "node:assert";
import { glob } from "./dist/main.js";

(async () => {
	assert.strictEqual((await glob["./bar.css"]()).default, "bar");
})();
