function foo() {
	return ['x', 'y'];
}

const result = [];

for (let a = foo(), i = 0; i < a.length; ++i) {
	const foo = a[i];
	result.push(foo);
}

assert.deepEqual(result, ['x', 'y']);