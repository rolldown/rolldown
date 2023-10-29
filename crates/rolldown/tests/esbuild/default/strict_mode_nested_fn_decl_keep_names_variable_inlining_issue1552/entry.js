export function outer() {
	{
		function inner() {
			return Math.random();
		}
		const x = inner();
		console.log(x);
	}
}
outer();