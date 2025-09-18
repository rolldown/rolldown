import fs from 'node:fs'
import assert from 'node:assert';
import path from 'path'


const file = fs.readFileSync(path.resolve(import.meta.dirname, "./dist/main.js"), "utf-8");

// after inline, original declaration should not be included in final bundle  
assert.ok(!file.includes("const"));
assert.ok(file.includes(`assert.equal("cjs-foo", "cjs-foo")`));
// TODO: enable it
// assert(!file.includes(`assert.equal("1.0.0", "1.0.0")`));

