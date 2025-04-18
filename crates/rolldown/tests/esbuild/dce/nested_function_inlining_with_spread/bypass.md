# Reason
1. Could be done in minifier
# Diff
## /out/entry.js
### esbuild
```js
// entry.js
function identity1(x) {
  return x;
}
function identity3(x) {
  return x;
}
check(
  void 0,
  (args, void 0),
  ([...args], void 0),
  identity1(),
  args,
  identity3(...args)
);
```
### rolldown
```js
//#region entry.js
function empty1() {}
function empty2() {}
function empty3() {}
function identity1(x) {
	return x;
}
function identity2(x) {
	return x;
}
function identity3(x) {
	return x;
}
check(empty1(), empty2(args), empty3(...args), identity1(), identity2(args), identity3(...args));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,7 +1,13 @@
+function empty1() {}
+function empty2() {}
+function empty3() {}
 function identity1(x) {
     return x;
 }
+function identity2(x) {
+    return x;
+}
 function identity3(x) {
     return x;
 }
-check(void 0, (args, void 0), ([...args], void 0), identity1(), args, identity3(...args));
+check(empty1(), empty2(args), empty3(...args), identity1(), identity2(args), identity3(...args));

```
## /out/entry-outer.js
### esbuild
```js
// inner.js
function identity1(x) {
  return x;
}
function identity3(x) {
  return x;
}

// entry-outer.js
check(
  void 0,
  (args, void 0),
  ([...args], void 0),
  identity1(),
  args,
  identity3(...args)
);
```
### rolldown
```js
//#region inner.js
function empty1() {}
function empty2() {}
function empty3() {}
function identity1(x) {
	return x;
}
function identity2(x) {
	return x;
}
function identity3(x) {
	return x;
}

//#endregion
//#region entry-outer.js
check(empty1(), empty2(args), empty3(...args), identity1(), identity2(args), identity3(...args));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry-outer.js
+++ rolldown	entry-outer.js
@@ -1,7 +1,13 @@
+function empty1() {}
+function empty2() {}
+function empty3() {}
 function identity1(x) {
     return x;
 }
+function identity2(x) {
+    return x;
+}
 function identity3(x) {
     return x;
 }
-check(void 0, (args, void 0), ([...args], void 0), identity1(), args, identity3(...args));
+check(empty1(), empty2(args), empty3(...args), identity1(), identity2(args), identity3(...args));

```