'use strict';

const shared = 'shared';

const unused = 'unused';
const dynamic = Promise.resolve().then(function () { return require('./generated-dynamic.js'); });

globalThis.sharedStatic = shared;

exports.dynamic = dynamic;
exports.shared = shared;
exports.unused = unused;
