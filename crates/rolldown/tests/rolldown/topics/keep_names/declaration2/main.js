import * as namespace from "./a";
import assert from 'node:assert'
console.log(`namespace: `, namespace);

var delay = function (time) {};
delay();
function random64() {}
random64();

assert.strictEqual(namespace.delay.name, "delay")
assert.strictEqual(namespace.random64.name, "random64")

assert.strictEqual(delay.name, "delay")
assert.strictEqual(random64.name, "random64")
