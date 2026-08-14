async function test(b) {
	for await (const a of b) a()
}