define(['exports', 'external1', 'external2'], (function (exports, external1, external2) { 'use strict';

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

	function _mergeNamespaces(n, m) {
		m.forEach(function (e) {
			e && typeof e !== 'string' && !Array.isArray(e) && Object.keys(e).forEach(function (k) {
				if (k !== 'default' && !(k in n)) {
					var d = Object.getOwnPropertyDescriptor(e, k);
					Object.defineProperty(n, k, d.get ? d : {
						enumerable: true,
						get: function () { return e[k]; }
					});
				}
			});
		});
		return Object.freeze(n);
	}

	var external1__namespace = /*#__PURE__*/_interopNamespaceDefault(external1);
	var external2__namespace = /*#__PURE__*/_interopNamespaceDefault(external2);

	var reexportExternal = /*#__PURE__*/_mergeNamespaces({
		__proto__: null
	}, [external1__namespace]);

	const extra = 'extra';

	const override = 'override';
	var reexportExternalsWithOverride = { synthetic: 'synthetic' };

	var reexportExternalsWithOverride$1 = /*#__PURE__*/_mergeNamespaces({
		__proto__: null,
		default: reexportExternalsWithOverride,
		extra: extra,
		override: override
	}, [reexportExternalsWithOverride, external1__namespace, external2__namespace]);

	exports.external = reexportExternal;
	exports.externalOverride = reexportExternalsWithOverride$1;

}));
