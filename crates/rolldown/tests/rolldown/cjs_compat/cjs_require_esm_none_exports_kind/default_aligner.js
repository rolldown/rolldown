// CJS module that require's aligner (ExportsKind::None module)
var Aligner = require('./aligner');
var DefaultAligner = function () {};
DefaultAligner.prototype = Object.create(Aligner);
module.exports = DefaultAligner;
