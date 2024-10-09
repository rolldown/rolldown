# Diff
## /out/a.js
### esbuild
```js
// a.js
var require_a = __commonJS({
  "a.js"(exports, module) {
    var foo = { bar: 123 };
    module.exports = foo;
  }
});
export default require_a();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/a.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var require_a = __commonJS({
-    "a.js"(exports, module) {
-        var foo = {
-            bar: 123
-        };
-        module.exports = foo;
-    }
-});
-export default require_a();

```
## /out/b.js
### esbuild
```js
// b.js
var require_b = __commonJS({
  "b.js"(exports, module) {
    var exports = { bar: 123 };
    module.exports = exports;
  }
});
export default require_b();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/b.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var require_b = __commonJS({
-    "b.js"(exports, module) {
-        var exports = {
-            bar: 123
-        };
-        module.exports = exports;
-    }
-});
-export default require_b();

```
## /out/c.js
### esbuild
```js
// c.js
var require_c = __commonJS({
  "c.js"(exports, module) {
    var module = { bar: 123 };
    exports.foo = module;
  }
});
export default require_c();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/c.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var require_c = __commonJS({
-    "c.js"(exports, module) {
-        var module = {
-            bar: 123
-        };
-        exports.foo = module;
-    }
-});
-export default require_c();

```