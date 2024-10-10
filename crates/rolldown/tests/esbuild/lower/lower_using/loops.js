for (using a of b) c(() => a)
for (await using d of e) f(() => d)
for await (using g of h) i(() => g)
for await (await using j of k) l(() => j)

if (nested) {
	for (using a of b) c(() => a)
	for (await using d of e) f(() => d)
	for await (using g of h) i(() => g)
	for await (await using j of k) l(() => j)
}

function foo() {
	for (using a of b) c(() => a)
}

async function bar() {
	for (using a of b) c(() => a)
	for (await using d of e) f(() => d)
	for await (using g of h) i(() => g)
	for await (await using j of k) l(() => j)
}