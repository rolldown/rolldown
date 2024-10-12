# Diff
## /out/top-level.js
### esbuild
```js
const n_keep = null, u_keep = void 0, i_keep = 1234567, f_keep = 123.456, s_keep = "";
console.log(
  // These are doubled to avoid the "inline const/let into next statement if used once" optimization
  null,
  null,
  void 0,
  void 0,
  1234567,
  1234567,
  123.456,
  123.456,
  s_keep,
  s_keep
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/top-level.js
+++ rolldown	
@@ -1,2 +0,0 @@
-const n_keep = null, u_keep = void 0, i_keep = 1234567, f_keep = 123.456, s_keep = "";
-console.log(null, null, void 0, void 0, 1234567, 1234567, 123.456, 123.456, s_keep, s_keep);

```
## /out/nested-block.js
### esbuild
```js
{
  const s_keep = "";
  console.log(
    // These are doubled to avoid the "inline const/let into next statement if used once" optimization
    null,
    null,
    void 0,
    void 0,
    1234567,
    1234567,
    123.456,
    123.456,
    s_keep,
    s_keep
  );
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-block.js
+++ rolldown	
@@ -1,4 +0,0 @@
-{
-    const s_keep = "";
-    console.log(null, null, void 0, void 0, 1234567, 1234567, 123.456, 123.456, s_keep, s_keep);
-}

```
## /out/nested-function.js
### esbuild
```js
function nested() {
  const s_keep = "";
  console.log(
    // These are doubled to avoid the "inline const/let into next statement if used once" optimization
    null,
    null,
    void 0,
    void 0,
    1234567,
    1234567,
    123.456,
    123.456,
    s_keep,
    s_keep
  );
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-function.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function nested() {
-    const s_keep = "";
-    console.log(null, null, void 0, void 0, 1234567, 1234567, 123.456, 123.456, s_keep, s_keep);
-}

```
## /out/namespace-export.js
### esbuild
```js
var ns;
((ns2) => (ns2.y_keep = 2, console.log(
  1,
  1,
  2,
  2
)))(ns ||= {});
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/namespace-export.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var ns;
-(ns2 => (ns2.y_keep = 2, console.log(1, 1, 2, 2)))(ns ||= {});

```
## /out/comment-before.js
### esbuild
```js
{
  //! comment
  x = [1, 1];
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/comment-before.js
+++ rolldown	
@@ -1,3 +0,0 @@
-{
-    x = [1, 1];
-}

```
## /out/directive-before.js
### esbuild
```js
function nested() {
  "directive";
  x = [1, 1];
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/directive-before.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function nested() {
-    "directive";
-    x = [1, 1];
-}

```
## /out/semicolon-before.js
### esbuild
```js
x = [1, 1];
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/semicolon-before.js
+++ rolldown	
@@ -1,1 +0,0 @@
-x = [1, 1];

```
## /out/debugger-before.js
### esbuild
```js
{
  debugger;
  x = [1, 1];
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/debugger-before.js
+++ rolldown	
@@ -1,4 +0,0 @@
-{
-    debugger;
-    x = [1, 1];
-}

```
## /out/type-before.js
### esbuild
```js
x = [1, 1];
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/type-before.js
+++ rolldown	
@@ -1,1 +0,0 @@
-x = [1, 1];

```
## /out/exprs-before.js
### esbuild
```js
function nested() {
  const x = [, "", {}, 0n, /./, function() {
  }, () => {
  }];
  function foo() {
    return 1;
  }
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/exprs-before.js
+++ rolldown	
@@ -1,6 +0,0 @@
-function nested() {
-    const x = [, "", {}, 0n, /./, function () {}, () => {}];
-    function foo() {
-        return 1;
-    }
-}

```
## /out/disabled-tdz.js
### esbuild
```js
foo();
const x_keep = 1;
function foo() {
  return x_keep;
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/disabled-tdz.js
+++ rolldown	
@@ -1,5 +0,0 @@
-foo();
-const x_keep = 1;
-function foo() {
-    return x_keep;
-}

```
## /out/backwards-reference-top-level.js
### esbuild
```js
const x = y, y = 1;
console.log(
  x,
  x,
  y,
  y
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/backwards-reference-top-level.js
+++ rolldown	
@@ -1,2 +0,0 @@
-const x = y, y = 1;
-console.log(x, x, y, y);

```
## /out/backwards-reference-nested-function.js
### esbuild
```js
function foo() {
  const x = y, y = 1;
  console.log(
    x,
    x,
    y,
    y
  );
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/backwards-reference-nested-function.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function foo() {
-    const x = y, y = 1;
-    console.log(x, x, y, y);
-}

```
## /out/issue-3125.js
### esbuild
```js
function foo() {
  const f = () => x, x = 0;
  return f();
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/issue-3125.js
+++ rolldown	
@@ -1,4 +0,0 @@
-function foo() {
-    const f = () => x, x = 0;
-    return f();
-}

```