System.register(['./custom_modules/@my-scope/my-base-pkg/index.js', './_virtual/index.js'], (function (exports) {
	'use strict';
	var myBasePkg;
	return {
		setters: [null, function (module) {
			myBasePkg = module.__exports;
		}],
		execute: (function () {

			const base = myBasePkg;

			var underBuild = exports('default', {
				base
			});

		})
	};
}));
