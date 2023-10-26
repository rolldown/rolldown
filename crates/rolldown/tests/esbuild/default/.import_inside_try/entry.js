let x
try {
	x = import('nope1')
	x = await import('nope2')
} catch {
}