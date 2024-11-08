let foo = 1
let bar = 2
let baz = 3
export {
	foo as 'import.meta',
	bar as 'import.meta.foo',
	baz as 'import.meta.foo.bar',
}