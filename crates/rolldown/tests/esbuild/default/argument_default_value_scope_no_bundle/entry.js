export function a(x = foo) { var foo; return x }
export class b { fn(x = foo) { var foo; return x } }
export let c = [
	function(x = foo) { var foo; return x },
	(x = foo) => { var foo; return x },
	{ fn(x = foo) { var foo; return x }},
	class { fn(x = foo) { var foo; return x }},
]