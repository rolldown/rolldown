var require_demo_pkg_index = __commonJSMin({
	"node_modules/demo-pkg/index.js"(exports) {
		exports.foo = 123;
		console.log("hello");
	},
});

//#endregion
//#region src/entry.js
require_demo_pkg_index();
console.log("unused import");
