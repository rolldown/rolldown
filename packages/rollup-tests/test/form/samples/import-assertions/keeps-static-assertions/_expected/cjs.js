'use strict';

var a = require('a');
var b = require('b');
var c = require('c');
var d$1 = require('d');
require('unresolved');

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

var b__namespace = /*#__PURE__*/_interopNamespaceDefault(b);

console.log(a.a, b__namespace, d);

Object.defineProperty(exports, 'c', {
	enumerable: true,
	get: function () { return c.c; }
});
Object.keys(d$1).forEach(function (k) {
	if (k !== 'default' && !exports.hasOwnProperty(k)) Object.defineProperty(exports, k, {
		enumerable: true,
		get: function () { return d$1[k]; }
	});
});
