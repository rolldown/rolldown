## /out/entry.js
### esbuild
```js
// entry.js
import("./foo-X6C7FV5C.js").then(({ default: { bar } }) => console.log(bar));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import("./foo-X6C7FV5C.js").then(({default: {bar}}) => console.log(bar));

```
## /out/foo-X6C7FV5C.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.bar = 123;
  }
});
export default require_foo();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-X6C7FV5C.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var require_foo = __commonJS({
-    "foo.js"(exports) {
-        exports.bar = 123;
-    }
-});
-export default require_foo();

```
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
import("./foo-X6C7FV5C.js").then(({ default: { bar } }) => console.log(bar));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,1 +0,0 @@
-import("./foo-X6C7FV5C.js").then(({default: {bar}}) => console.log(bar));

```
## /out/foo-X6C7FV5C.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"(exports) {
    exports.bar = 123;
  }
});
export default require_foo();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/foo-X6C7FV5C.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var require_foo = __commonJS({
-    "foo.js"(exports) {
-        exports.bar = 123;
-    }
-});
-export default require_foo();

```