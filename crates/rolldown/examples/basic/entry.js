import test from "node:test"
import assert from "node:assert"



test("@peculiar/webcrypto", () => import("./a").then(assert.ok));


test("webcrypto-core", () => import("./b").then(assert.ok));
