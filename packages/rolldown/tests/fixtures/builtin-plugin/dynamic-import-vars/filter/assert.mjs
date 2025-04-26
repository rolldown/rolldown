// @ts-nocheck
import assert from "node:assert";
import { dynamicImport } from "./dist/main";

async function test() {
  try {
    await dynamicImport("a");
  } catch (e) {
    return assert.strictEqual(
      e.message,
      "Unknown variable dynamic import: ./mod/a.js"
    );
  }
  assert.fail('Should be exclueded');
}

test();