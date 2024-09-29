export function confuseNode(exports) {
	// If this local is called "exports", node incorrectly
	// thinks this file has an export called "notAnExport".
	// We must make sure that it doesn't have that name
	// when targeting Node with CommonJS.
	exports.notAnExport = function() {
	};
}