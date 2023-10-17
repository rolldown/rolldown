'use strict';

const other = require('other');

const a = 1;
const b = 2;

const namespace = /*#__PURE__*/Object.freeze({
	__proto__: null,
	a: a,
	b: b
});

console.log( Object.keys( namespace ) );
console.log( other.name );

const main = 42;

module.exports = main;
