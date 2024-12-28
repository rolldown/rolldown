import { es2019 } from './dist/main'
import assert from 'node:assert'

assert.strictEqual(es2019.toString(), `function es2019() {
	let temp;
	try {
		temp = JSON.parse("[1, 2, [3]]");
	} catch {}
	console.log(temp);
	console.log([
		1,
		2,
		[3]
	].flat());
}`)
