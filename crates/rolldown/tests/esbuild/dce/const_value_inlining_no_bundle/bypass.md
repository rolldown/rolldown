# Reason
1. const inline could be done in minifier
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
//#region top-level.js
const n_keep = null;
const u_keep = void 0;
const i_keep = 1234567;
const f_keep = 123.456;
const s_keep = "";
console.log(
	// These are doubled to avoid the "inline const/let into next statement if used once" optimization
	n_keep,
	n_keep,
	u_keep,
	u_keep,
	i_keep,
	i_keep,
	f_keep,
	f_keep,
	s_keep,
	s_keep
);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/top-level.js
+++ rolldown	top-level.js
@@ -1,2 +1,6 @@
-const n_keep = null, u_keep = void 0, i_keep = 1234567, f_keep = 123.456, s_keep = "";
-console.log(null, null, void 0, void 0, 1234567, 1234567, 123.456, 123.456, s_keep, s_keep);
+var n_keep = null;
+var u_keep = void 0;
+var i_keep = 1234567;
+var f_keep = 123.456;
+var s_keep = "";
+console.log(n_keep, n_keep, u_keep, u_keep, i_keep, i_keep, f_keep, f_keep, s_keep, s_keep);

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
//#region nested-block.js
{
	const REMOVE_n = null;
	const REMOVE_u = void 0;
	const REMOVE_i = 1234567;
	const REMOVE_f = 123.456;
	const s_keep = "";
	console.log(
		// These are doubled to avoid the "inline const/let into next statement if used once" optimization
		REMOVE_n,
		REMOVE_n,
		REMOVE_u,
		REMOVE_u,
		REMOVE_i,
		REMOVE_i,
		REMOVE_f,
		REMOVE_f,
		s_keep,
		s_keep
);
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/nested-block.js
+++ rolldown	nested-block.js
@@ -1,4 +1,8 @@
 {
+    const REMOVE_n = null;
+    const REMOVE_u = void 0;
+    const REMOVE_i = 1234567;
+    const REMOVE_f = 123.456;
     const s_keep = "";
-    console.log(null, null, void 0, void 0, 1234567, 1234567, 123.456, 123.456, s_keep, s_keep);
+    console.log(REMOVE_n, REMOVE_n, REMOVE_u, REMOVE_u, REMOVE_i, REMOVE_i, REMOVE_f, REMOVE_f, s_keep, s_keep);
 }

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
//#region nested-function.js
function nested() {
	const REMOVE_n = null;
	const REMOVE_u = void 0;
	const REMOVE_i = 1234567;
	const REMOVE_f = 123.456;
	const s_keep = "";
	console.log(
		// These are doubled to avoid the "inline const/let into next statement if used once" optimization
		REMOVE_n,
		REMOVE_n,
		REMOVE_u,
		REMOVE_u,
		REMOVE_i,
		REMOVE_i,
		REMOVE_f,
		REMOVE_f,
		s_keep,
		s_keep
);
}
assert(nested() !== void 0);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/nested-function.js
+++ rolldown	nested-function.js
@@ -1,4 +1,9 @@
 function nested() {
+    const REMOVE_n = null;
+    const REMOVE_u = void 0;
+    const REMOVE_i = 1234567;
+    const REMOVE_f = 123.456;
     const s_keep = "";
-    console.log(null, null, void 0, void 0, 1234567, 1234567, 123.456, 123.456, s_keep, s_keep);
+    console.log(REMOVE_n, REMOVE_n, REMOVE_u, REMOVE_u, REMOVE_i, REMOVE_i, REMOVE_f, REMOVE_f, s_keep, s_keep);
 }
+assert(nested() !== void 0);

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
//#region namespace-export.ts
let ns;
(function(_ns) {
	const x_REMOVE = 1;
	const y_keep = _ns.y_keep = 2;
	console.log(x_REMOVE, x_REMOVE, y_keep, y_keep);
})(ns || (ns = {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/namespace-export.js
+++ rolldown	namespace-export.js
@@ -1,2 +1,6 @@
 var ns;
-(ns2 => (ns2.y_keep = 2, console.log(1, 1, 2, 2)))(ns ||= {});
+(function (_ns) {
+    const x_REMOVE = 1;
+    const y_keep = _ns.y_keep = 2;
+    console.log(x_REMOVE, x_REMOVE, y_keep, y_keep);
+})(ns || (ns = {}));

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
//#region comment-before.js
{
	//! comment
	const REMOVE = 1;
	x = [REMOVE, REMOVE];
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/comment-before.js
+++ rolldown	comment-before.js
@@ -1,3 +1,4 @@
 {
-    x = [1, 1];
+    const REMOVE = 1;
+    x = [REMOVE, REMOVE];
 }

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
//#region directive-before.js
function nested() {
	"directive";
	const REMOVE = 1;
	x = [REMOVE, REMOVE];
}
assert(nested() !== void 0);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/directive-before.js
+++ rolldown	directive-before.js
@@ -1,4 +1,6 @@
 function nested() {
     "directive";
-    x = [1, 1];
+    const REMOVE = 1;
+    x = [REMOVE, REMOVE];
 }
+assert(nested() !== void 0);

```
## /out/semicolon-before.js
### esbuild
```js
x = [1, 1];
```
### rolldown
```js
//#region semicolon-before.js
{
	const REMOVE = 1;
	x = [REMOVE, REMOVE];
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/semicolon-before.js
+++ rolldown	semicolon-before.js
@@ -1,1 +1,4 @@
-x = [1, 1];
+{
+    const REMOVE = 1;
+    x = [REMOVE, REMOVE];
+}

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
//#region debugger-before.js
{
	debugger;
	const REMOVE = 1;
	x = [REMOVE, REMOVE];
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/debugger-before.js
+++ rolldown	debugger-before.js
@@ -1,4 +1,5 @@
 {
     debugger;
-    x = [1, 1];
+    const REMOVE = 1;
+    x = [REMOVE, REMOVE];
 }

```
## /out/type-before.js
### esbuild
```js
x = [1, 1];
```
### rolldown
```js
//#region type-before.ts
{
	const REMOVE = 1;
	x = [REMOVE, REMOVE];
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/type-before.js
+++ rolldown	type-before.js
@@ -1,1 +1,4 @@
-x = [1, 1];
+{
+    const REMOVE = 1;
+    x = [REMOVE, REMOVE];
+}

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
//#region exprs-before.js
function nested() {
	const x = [
		,
		"",
		{},
		0n,
		/./,
		function() {},
		() => {}
	];
	const y_REMOVE = 1;
	function foo() {
		return y_REMOVE;
	}
}
assert(nested() !== void 0);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/exprs-before.js
+++ rolldown	exprs-before.js
@@ -1,6 +1,8 @@
 function nested() {
     const x = [, "", {}, 0n, /./, function () {}, () => {}];
+    const y_REMOVE = 1;
     function foo() {
-        return 1;
+        return y_REMOVE;
     }
 }
+assert(nested() !== void 0);

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
//#region disabled-tdz.js
foo();
const x_keep = 1;
function foo() {
	return x_keep;
}

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/disabled-tdz.js
+++ rolldown	disabled-tdz.js
@@ -1,5 +1,5 @@
 foo();
-const x_keep = 1;
+var x_keep = 1;
 function foo() {
     return x_keep;
 }

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
//#region backwards-reference-top-level.js
const x = y;
const y = 1;
console.log(x, x, y, y);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/backwards-reference-top-level.js
+++ rolldown	backwards-reference-top-level.js
@@ -1,2 +1,3 @@
-const x = y, y = 1;
+var x = y;
+var y = 1;
 console.log(x, x, y, y);

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
//#region backwards-reference-nested-function.js
function foo() {
	const x = y;
	const y = 1;
	console.log(x, x, y, y);
}
assert(foo() !== void 0);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/backwards-reference-nested-function.js
+++ rolldown	backwards-reference-nested-function.js
@@ -1,4 +1,6 @@
 function foo() {
-    const x = y, y = 1;
+    const x = y;
+    const y = 1;
     console.log(x, x, y, y);
 }
+assert(foo() !== void 0);

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
//#region issue-3125.js
function foo() {
	const f = () => x;
	const x = 0;
	return f();
}
assert(foo() !== void 0);

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/issue-3125.js
+++ rolldown	issue-3125.js
@@ -1,4 +1,6 @@
 function foo() {
-    const f = () => x, x = 0;
+    const f = () => x;
+    const x = 0;
     return f();
 }
+assert(foo() !== void 0);

```