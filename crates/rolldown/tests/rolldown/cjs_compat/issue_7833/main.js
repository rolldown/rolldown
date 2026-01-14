import config from "./config.js";
import assert from "node:assert";

assert.deepStrictEqual(config, { name: "example" });
