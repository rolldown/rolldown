# Reason
1. not support require second argument
2. wrong `export default require_entry()`;
# Diff
## /out/entry.js
### esbuild
```js
// example.json
var require_example = __commonJS({
  "example.json"(exports, module) {
    module.exports = { works: true };
  }
});

// entry.js
console.log([
  __require,
  typeof __require,
  require_example(),
  __require("./example.json", { type: "json" }),
  __require(window.SOME_PATH),
  require_example(),
  __require("./example.json", { type: "json" }),
  __require(window.SOME_PATH),
  __require.resolve("some-path"),
  __require.resolve(window.SOME_PATH),
  Promise.resolve().then(() => __toESM(__require("some-path"))),
  Promise.resolve().then(() => __toESM(__require(window.SOME_PATH)))
]);
```
### rolldown
```js

//#region example.json
var require_example = __commonJS({ "example.json"(exports, module) {
	module.exports = { "works": true };
} });

//#endregion
//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports, module) {
	console.log([
		__require,
		typeof __require,
		require_example(),
		require_example(),
		__require(window.SOME_PATH),
		module.require("./example.json"),
		module.require("./example.json", { type: "json" }),
		module.require(window.SOME_PATH),
		__require.resolve("some-path"),
		__require.resolve(window.SOME_PATH),
		import("some-path"),
		import(window.SOME_PATH)
	]);
} });

//#endregion
export default require_entry();

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,12 +1,15 @@
 var require_example = __commonJS({
     "example.json"(exports, module) {
         module.exports = {
-            works: true
+            "works": true
         };
     }
 });
-console.log([__require, typeof __require, require_example(), __require("./example.json", {
-    type: "json"
-}), __require(window.SOME_PATH), require_example(), __require("./example.json", {
-    type: "json"
-}), __require(window.SOME_PATH), __require.resolve("some-path"), __require.resolve(window.SOME_PATH), Promise.resolve().then(() => __toESM(__require("some-path"))), Promise.resolve().then(() => __toESM(__require(window.SOME_PATH)))]);
+var require_entry = __commonJS({
+    "entry.js"(exports, module) {
+        console.log([__require, typeof __require, require_example(), require_example(), __require(window.SOME_PATH), module.require("./example.json"), module.require("./example.json", {
+            type: "json"
+        }), module.require(window.SOME_PATH), __require.resolve("some-path"), __require.resolve(window.SOME_PATH), import("some-path"), import(window.SOME_PATH)]);
+    }
+});
+export default require_entry();

```