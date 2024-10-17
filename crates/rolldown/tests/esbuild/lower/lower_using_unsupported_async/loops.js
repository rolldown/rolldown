for (using a of b) c(() => a)

if (nested) {
	for (using a of b) c(() => a)
}

function foo() {
	for (using a of b) c(() => a)
}

async function bar() {
	for (using a of b) c(() => a)
	for (await using d of e) f(() => d)
}