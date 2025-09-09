import fs from 'node:fs'
import assert from 'node:assert';
import path from 'path'


const file = fs.readFileSync(path.resolve(import.meta.dirname, "./dist/main.js"), "utf-8");

assert.ok(!file.includes("unused"));

