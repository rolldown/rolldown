import fs from 'node:fs'
import assert from 'node:assert';
import path from 'path'

const file = fs.readFileSync(path.resolve(import.meta.dirname, "./dist/main.js"), "utf-8");

// Check that unused exports are not included
assert.ok(!file.includes("unused1"), "unused1 should be tree-shaken");
assert.ok(!file.includes("unused2"), "unused2 should be tree-shaken");

// Check that used export is included
assert.ok(file.includes("used"), "used should be included");
