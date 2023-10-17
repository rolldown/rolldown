System.register('bundle', ['external'], (function (exports) {
	'use strict';
	var _starExcludes = {
		default: 1
	};
	return {
		setters: [function (module) {
			var setter = { default: module.default };
			for (var name in module) {
				if (!_starExcludes[name]) setter[name] = module[name];
			}
			exports(setter);
		}],
		execute: (function () {



		})
	};
}));
