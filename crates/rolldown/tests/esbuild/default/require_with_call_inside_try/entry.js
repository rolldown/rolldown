try {
	const supportsColor = require('supports-color');
	if (supportsColor && (supportsColor.stderr || supportsColor).level >= 2) {
		exports.colors = [];
	}
} catch (error) {
}