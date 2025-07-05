import fs from 'node:fs'
import assert from 'node:assert';
import path from 'path'


const pageA = fs.readFileSync(path.resolve(import.meta.dirname, "./dist/page-a.js"), "utf-8");
const pageB = fs.readFileSync(path.resolve(import.meta.dirname, "./dist/page-b.js"), "utf-8");

// after inline, original declaration should not be included in final bundle  
assert.ok(!pageA.includes("page-a"));
assert.ok(!pageB.includes("page-b"));

