import * as external1 from 'external1';
import * as external2 from 'external2';

function _mergeNamespaces(n, m) {
	for (var i = 0; i < m.length; i++) {
		var e = m[i];
		if (typeof e !== 'string' && !Array.isArray(e)) { for (var k in e) {
			if (k !== 'default' && !(k in n)) {
				n[k] = e[k];
			}
		} }
	}
	return Object.freeze(n);
}

const __synthetic$1 = { module: 'synthetic' };

const __synthetic = { module: 'reexport' };

var ns = /*#__PURE__*/_mergeNamespaces({
	__proto__: null
}, [__synthetic, __synthetic$1, external1, external2]);

console.log(ns);
