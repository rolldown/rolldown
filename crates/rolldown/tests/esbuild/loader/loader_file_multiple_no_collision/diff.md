# Reason
1. Not support file loader
# Diff
## /dist/test-J7OMUXO3.txt
### esbuild
```js
test
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/dist/test-J7OMUXO3.txt
+++ rolldown	
@@ -1,1 +0,0 @@
-test;

```
## /dist/out.js
### esbuild
```js
// a/test.txt
var require_test = __commonJS({
  "a/test.txt"(exports, module) {
    module.exports = "./test-J7OMUXO3.txt";
  }
});

// b/test.txt
var require_test2 = __commonJS({
  "b/test.txt"(exports, module) {
    module.exports = "./test-J7OMUXO3.txt";
  }
});

// entry.js
console.log(
  require_test(),
  require_test2()
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/dist/out.js
+++ rolldown	
@@ -1,11 +0,0 @@
-var require_test = __commonJS({
-    "a/test.txt"(exports, module) {
-        module.exports = "./test-J7OMUXO3.txt";
-    }
-});
-var require_test2 = __commonJS({
-    "b/test.txt"(exports, module) {
-        module.exports = "./test-J7OMUXO3.txt";
-    }
-});
-console.log(require_test(), require_test2());

```