import { es2015 } from './dist/main'
import assert from 'node:assert'

assert.strictEqual(es2015.toString(), `function es2015() {
	let temp;
	try {
		temp = JSON.parse("[1, 2, [3]]");
	} catch (_unused) {}
	console.log(temp);
	console.log([
		1,
		2,
		[3]
	].flat());
}`)
