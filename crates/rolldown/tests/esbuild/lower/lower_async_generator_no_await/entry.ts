async function* foo() {
	yield
	yield x
	yield *x
	await using x = await y
	for await (let x of y) {}
	for await (await using x of y) {}
}
foo = async function* () {
	yield
	yield x
	yield *x
	await using x = await y
	for await (let x of y) {}
	for await (await using x of y) {}
}
foo = { async *bar () {
	yield
	yield x
	yield *x
	await using x = await y
	for await (let x of y) {}
	for await (await using x of y) {}
} }
class Foo { async *bar () {
	yield
	yield x
	yield *x
	await using x = await y
	for await (let x of y) {}
	for await (await using x of y) {}
} }
Foo = class { async *bar () {
	yield
	yield x
	yield *x
	await using x = await y
	for await (let x of y) {}
	for await (await using x of y) {}
} }
async function bar() {
	await using x = await y
	for await (let x of y) {}
	for await (await using x of y) {}
}