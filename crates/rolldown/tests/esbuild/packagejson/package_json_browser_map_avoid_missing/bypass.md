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

//#region rolldown:runtime
var __getOwnPropNames = Object.getOwnPropertyNames;
var __commonJS = (cb, mod) => function() {
	return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports;
};

//#region node_modules/component-indexof/index.js
var require_component_indexof = __commonJS({ "node_modules/component-indexof/index.js"(exports, module) {
	module.exports = function() {
		return 234;
	};
} });

//#region node_modules/component-classes/index.js
try {
	var index = require_component_indexof();
} catch (err) {
	var index = require_component_indexof();
}

```
### diff
```diff
===================================================================
--- esbuild	/Users/user/project/out.js
+++ rolldown	entry.js
@@ -1,13 +1,18 @@
+var __getOwnPropNames = Object.getOwnPropertyNames;
+var __commonJS = (cb, mod) => function () {
+    return (mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = {
+        exports: {}
+    }).exports, mod), mod.exports);
+};
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
+    var index = require_component_indexof();
 } catch (err) {
-    index = require_component_indexof();
+    var index = require_component_indexof();
 }
-var index;

```