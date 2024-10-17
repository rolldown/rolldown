# Reason
1. require `.json`, the json file should not wrapped in `__esm`
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
var example_exports = {};
__export(example_exports, {
	default: () => example_default,
	works: () => works
});
var works, example_default;
var init_example = __esm({ "example.json"() {
	works = true;
	example_default = { works };
} });

//#endregion
//#region entry.js
var require_entry = __commonJS({ "entry.js"(exports, module) {
	console.log([
		require,
		typeof require,
		(init_example(), __toCommonJS(example_exports).default),
		(init_example(), __toCommonJS(example_exports).default),
		require(window.SOME_PATH),
		module.require("./example.json"),
		module.require("./example.json", { type: "json" }),
		module.require(window.SOME_PATH),
		require.resolve("some-path"),
		require.resolve(window.SOME_PATH),
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
@@ -1,12 +1,22 @@
-var require_example = __commonJS({
-    "example.json"(exports, module) {
-        module.exports = {
-            works: true
+var example_exports = {};
+__export(example_exports, {
+    default: () => example_default,
+    works: () => works
+});
+var works, example_default;
+var init_example = __esm({
+    "example.json"() {
+        works = true;
+        example_default = {
+            works
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
+        console.log([require, typeof require, (init_example(), __toCommonJS(example_exports).default), (init_example(), __toCommonJS(example_exports).default), require(window.SOME_PATH), module.require("./example.json"), module.require("./example.json", {
+            type: "json"
+        }), module.require(window.SOME_PATH), require.resolve("some-path"), require.resolve(window.SOME_PATH), import("some-path"), import(window.SOME_PATH)]);
+    }
+});
+export default require_entry();

```