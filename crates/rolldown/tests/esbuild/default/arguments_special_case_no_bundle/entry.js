(() => {
	var arguments;

	function foo(x = arguments) { return arguments }
	(function(x = arguments) { return arguments });
	({foo(x = arguments) { return arguments }});
	class Foo { foo(x = arguments) { return arguments } }
	(class { foo(x = arguments) { return arguments } });

	function foo(x = arguments) { var arguments; return arguments }
	(function(x = arguments) { var arguments; return arguments });
	({foo(x = arguments) { var arguments; return arguments }});

	(x => arguments);
	(() => arguments);
	(async () => arguments);
	((x = arguments) => arguments);
	(async (x = arguments) => arguments);

	x => arguments;
	() => arguments;
	async () => arguments;
	(x = arguments) => arguments;
	async (x = arguments) => arguments;

	(x => { return arguments });
	(() => { return arguments });
	(async () => { return arguments });
	((x = arguments) => { return arguments });
	(async (x = arguments) => { return arguments });

	x => { return arguments };
	() => { return arguments };
	async () => { return arguments };
	(x = arguments) => { return arguments };
	async (x = arguments) => { return arguments };
})()