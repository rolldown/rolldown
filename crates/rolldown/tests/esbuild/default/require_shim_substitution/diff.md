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

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,12 +0,0 @@
-var require_example = __commonJS({
-    "example.json"(exports, module) {
-        module.exports = {
-            works: true
-        };
-    }
-});
-console.log([__require, typeof __require, require_example(), __require("./example.json", {
-    type: "json"
-}), __require(window.SOME_PATH), require_example(), __require("./example.json", {
-    type: "json"
-}), __require(window.SOME_PATH), __require.resolve("some-path"), __require.resolve(window.SOME_PATH), Promise.resolve().then(() => __toESM(__require("some-path"))), Promise.resolve().then(() => __toESM(__require(window.SOME_PATH)))]);

```