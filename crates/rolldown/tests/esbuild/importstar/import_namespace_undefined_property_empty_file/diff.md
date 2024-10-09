# Diff
## /out/entry-nope.js
### esbuild
```js
// empty.js
var require_empty = __commonJS({
  "empty.js"() {
  }
});

// empty.cjs
var require_empty2 = __commonJS({
  "empty.cjs"() {
  }
});

// entry-nope.js
var js = __toESM(require_empty());
var cjs = __toESM(require_empty2());
console.log(
  void 0,
  void 0,
  void 0
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry-nope.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var require_empty = __commonJS({
-    "empty.js"() {}
-});
-var require_empty2 = __commonJS({
-    "empty.cjs"() {}
-});
-var js = __toESM(require_empty());
-var cjs = __toESM(require_empty2());
-console.log(void 0, void 0, void 0);

```
## /out/entry-default.js
### esbuild
```js
// empty.js
var require_empty = __commonJS({
  "empty.js"() {
  }
});

// empty.cjs
var require_empty2 = __commonJS({
  "empty.cjs"() {
  }
});

// entry-default.js
var js = __toESM(require_empty());
var cjs = __toESM(require_empty2());
console.log(
  js.default,
  void 0,
  cjs.default
);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry-default.js
+++ rolldown	
@@ -1,9 +0,0 @@
-var require_empty = __commonJS({
-    "empty.js"() {}
-});
-var require_empty2 = __commonJS({
-    "empty.cjs"() {}
-});
-var js = __toESM(require_empty());
-var cjs = __toESM(require_empty2());
-console.log(js.default, void 0, cjs.default);

```