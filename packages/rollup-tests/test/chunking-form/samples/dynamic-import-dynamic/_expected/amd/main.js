define(['require'], (function (require) { 'use strict';

	function _interopNamespaceDefault(e) {
		var n = Object.create(null);
		if (e) {
			Object.keys(e).forEach(function (k) {
				if (k !== 'default') {
					var d = Object.getOwnPropertyDescriptor(e, k);
					Object.defineProperty(n, k, d.get ? d : {
						enumerable: true,
						get: function () { return e[k]; }
					});
				}
			});
		}
		n.default = e;
		return Object.freeze(n);
	}

	var dep = 'dep';

	(function (t) { return new Promise(function (resolve, reject) { require([t], function (m) { resolve(/*#__PURE__*/_interopNamespaceDefault(m)); }, reject); }); })(dep);

}));
