# Reason
1. different fs
2. related https://github.com/evanw/esbuild/commit/918d44e7e2912fa23f9ba409e1d6623275f7b83f#diff-e20508c4ae566a2d8a60274ff05e408d81c9758a27d84318feecdfbf9e24af5eR211-R216
3. sub optimal
# Diff
## /Users/user/project/out.js
### esbuild
```js
// Users/user/project/node_modules/component-indexof/index.js
var require_component_indexof = __commonJS({
  "Users/user/project/node_modules/component-indexof/index.js"(exports, module) {
    module.exports = function() {
      return 234;
    };
  }
});

// Users/user/project/node_modules/component-classes/index.js
try {
  index = require_component_indexof();
} catch (err) {
  index = require_component_indexof();
}
var index;
```
### rolldown
```js
// HIDDEN [rolldown:runtime]
//#region node_modules/component-indexof/index.js
var require_component_indexof = /* @__PURE__ */ __commonJS({ "node_modules/component-indexof/index.js"(exports, module) {
	module.exports = function() {
		return 234;
	};
} });

//#endregion
//#region node_modules/component-classes/index.js
try {
	require_component_indexof();
} catch (err) {
	require_component_indexof();
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,13 +1,12 @@
 var require_component_indexof = __commonJS({
-    "Users/user/project/node_modules/component-indexof/index.js"(exports, module) {
+    "node_modules/component-indexof/index.js"(exports, module) {
         module.exports = function () {
             return 234;
         };
     }
 });
 try {
-    index = require_component_indexof();
+    require_component_indexof();
 } catch (err) {
-    index = require_component_indexof();
+    require_component_indexof();
 }
-var index;

```