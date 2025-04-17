# Reason
1. Can finish in minifier
# Diff
## /out.js
### esbuild
```js
// entry.js
function foo() {
  a && b();
}
function bar() {
  return a && b(), KEEP_ME;
}
var entry_default = [
  foo,
  bar,
  function() {
    a && b();
  },
  function() {
    return a && b(), KEEP_ME;
  },
  () => {
    a && b();
  },
  () => (a && b(), KEEP_ME)
];
export {
  entry_default as default
};
```
### rolldown
```js

//#region entry.js
function foo() {
	if (a) b();
	return;
}
function bar() {
	if (a) b();
	return KEEP_ME;
}
var entry_default = [
	foo,
	bar,
	function() {
		if (a) b();
		return;
	},
	function() {
		if (a) b();
		return KEEP_ME;
	},
	() => {
		if (a) b();
		return;
	},
	() => {
		if (a) b();
		return KEEP_ME;
	}
];

export { entry_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,14 +1,22 @@
 function foo() {
-    a && b();
+    if (a) b();
+    return;
 }
 function bar() {
-    return (a && b(), KEEP_ME);
+    if (a) b();
+    return KEEP_ME;
 }
 var entry_default = [foo, bar, function () {
-    a && b();
+    if (a) b();
+    return;
 }, function () {
-    return (a && b(), KEEP_ME);
+    if (a) b();
+    return KEEP_ME;
 }, () => {
-    a && b();
-}, () => (a && b(), KEEP_ME)];
+    if (a) b();
+    return;
+}, () => {
+    if (a) b();
+    return KEEP_ME;
+}];
 export {entry_default as default};

```