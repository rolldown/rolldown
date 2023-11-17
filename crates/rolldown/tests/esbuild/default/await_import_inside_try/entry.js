async function main(name) {
	try {
		return await import(name)
	} catch {
	}
}
main('fs')