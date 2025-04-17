# Reason
1. different deconflict order
# Diff
## /out.js
### esbuild
```js
// folders/index.js
var folders_exports = {};
__export(folders_exports, {
  foo: () => foo
});

// folders/child/foo.js
var foo = () => "hi there";

// entry.js
console.log(JSON.stringify(folders_exports));
```
### rolldown
```js

//#region rolldown:runtime
var __defProp = Object.defineProperty;
var __export = (target, all) => {
	for (var name in all) __defProp(target, name, {
		get: all[name],
		enumerable: true
	});
};

//#region folders/child/foo.js
const foo = () => "hi there";

//#region folders/index.js
var folders_exports = {};
__export(folders_exports, { foo: () => foo });

//#region entry.js
console.log(JSON.stringify(folders_exports));

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,6 +1,13 @@
+var __defProp = Object.defineProperty;
+var __export = (target, all) => {
+    for (var name in all) __defProp(target, name, {
+        get: all[name],
+        enumerable: true
+    });
+};
+var foo = () => "hi there";
 var folders_exports = {};
 __export(folders_exports, {
     foo: () => foo
 });
-var foo = () => "hi there";
 console.log(JSON.stringify(folders_exports));

```