# Reason
1. sub optimal
2. eval in `test4` param position don't need to rename
# Diff
## /out.js
### esbuild
```js
function test1() {
  function add(n, t) {
    return n + t;
  }
  eval("add(1, 2)");
}
function test2() {
  function n(t, e) {
    return t + e;
  }
  (0, eval)("add(1, 2)");
}
function test3() {
  function n(t, e) {
    return t + e;
  }
}
function test4(eval) {
  function add(n, t) {
    return n + t;
  }
  eval("add(1, 2)");
}
function test5() {
  function containsDirectEval() {
    eval();
  }
  if (true) {
    var shouldNotBeRenamed;
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,31 +0,0 @@
-function test1() {
-  function add(n, t) {
-    return n + t;
-  }
-  eval("add(1, 2)");
-}
-function test2() {
-  function n(t, e) {
-    return t + e;
-  }
-  (0, eval)("add(1, 2)");
-}
-function test3() {
-  function n(t, e) {
-    return t + e;
-  }
-}
-function test4(eval) {
-  function add(n, t) {
-    return n + t;
-  }
-  eval("add(1, 2)");
-}
-function test5() {
-  function containsDirectEval() {
-    eval();
-  }
-  if (true) {
-    var shouldNotBeRenamed;
-  }
-}
\ No newline at end of file

```