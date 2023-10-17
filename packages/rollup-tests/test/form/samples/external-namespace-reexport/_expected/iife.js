var myBundle = (function (exports, highcharts) {
	'use strict';

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

	var highcharts__namespace = /*#__PURE__*/_interopNamespaceDefault(highcharts);



	exports.Highcharts = highcharts__namespace;

	return exports;

})({}, highcharts);
