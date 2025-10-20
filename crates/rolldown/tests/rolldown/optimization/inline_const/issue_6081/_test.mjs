import fs from "node:fs";
import assert from "node:assert";
import path from "node:path";

const file = fs.readFileSync(
  path.resolve(import.meta.dirname, "./dist/main.js"),
  "utf-8",
);

assert.ok(
  !file.includes("version.startsWith"),
  "expected the named import to be inlined rather than referencing version.startsWith",
);

assert.ok(
  file.includes(`"1.0.0".startsWith("1")`),
  "expected the constant string literal to replace the named import",
);

assert(
  file.includes(
    `assert.strictEqual(import_should_not_inline_import_default.default, 1e3)`,
  ),
);
