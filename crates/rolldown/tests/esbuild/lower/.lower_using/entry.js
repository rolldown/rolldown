using a = b
await using c = d
if (nested) {
	using x = 1
	await using y = 2
}

function foo() {
	using a = b
	if (nested) {
		using x = 1
	}
}

async function bar() {
	using a = b
	await using c = d
	if (nested) {
		using x = 1
		await using y = 2
	}
}