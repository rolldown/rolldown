import assert from 'node:assert';
import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const fixtureDir = path.dirname(fileURLToPath(import.meta.url));
const firstPath = path.join(fixtureDir, './dist/index.js');
const secondPath = path.join(fixtureDir, './dist/index2.js');
const first = await import(firstPath);
const second = await import(secondPath);

const appModule = 'appValue' in first ? first : second;
const serverModule = 'serverValue' in first ? first : second;

assert.strictEqual(appModule.appValue, 'module1');
assert.strictEqual(serverModule.serverValue, 'module2');
assert.ok(/\bappFactory\(/.test(appModule.App.toString()));

const firstCode = readFileSync(firstPath, 'utf8');
const secondCode = readFileSync(secondPath, 'utf8');
const appCode = firstCode.includes('appValue') ? firstCode : secondCode;
const serverCode = firstCode.includes('serverValue') ? firstCode : secondCode;

assert.ok(!/from ['"]1\/index['"]/.test(appCode));
assert.ok(!/from ['"]2\/index['"]/.test(serverCode));
