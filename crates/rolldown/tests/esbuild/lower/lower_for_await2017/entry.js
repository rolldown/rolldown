export default [
	async () => { for await (x of y) z(x) },
	async () => { for await (x.y of y) z(x) },
	async () => { for await (let x of y) z(x) },
	async () => { for await (const x of y) z(x) },
]