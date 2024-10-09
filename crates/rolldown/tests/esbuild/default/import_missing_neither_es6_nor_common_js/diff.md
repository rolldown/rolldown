# Diff
## /out/named.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"() {
    console.log("no exports here");
  }
});

// named.js
var import_foo = __toESM(require_foo());
console.log((0, import_foo.default)(void 0, void 0));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/named.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var require_foo = __commonJS({
-    "foo.js"() {
-        console.log("no exports here");
-    }
-});
-var import_foo = __toESM(require_foo());
-console.log((0, import_foo.default)(void 0, void 0));

```
## /out/star.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"() {
    console.log("no exports here");
  }
});

// star.js
var ns = __toESM(require_foo());
console.log(ns.default(void 0, void 0));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/star.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var require_foo = __commonJS({
-    "foo.js"() {
-        console.log("no exports here");
-    }
-});
-var ns = __toESM(require_foo());
-console.log(ns.default(void 0, void 0));

```
## /out/star-capture.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"() {
    console.log("no exports here");
  }
});

// star-capture.js
var ns = __toESM(require_foo());
console.log(ns);
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/star-capture.js
+++ rolldown	
@@ -1,7 +0,0 @@
-var require_foo = __commonJS({
-    "foo.js"() {
-        console.log("no exports here");
-    }
-});
-var ns = __toESM(require_foo());
-console.log(ns);

```
## /out/bare.js
### esbuild
```js
// foo.js
console.log("no exports here");
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/bare.js
+++ rolldown	
@@ -1,1 +0,0 @@
-console.log("no exports here");

```
## /out/require.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"() {
    console.log("no exports here");
  }
});

// require.js
console.log(require_foo());
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/require.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var require_foo = __commonJS({
-    "foo.js"() {
-        console.log("no exports here");
-    }
-});
-console.log(require_foo());

```
## /out/import.js
### esbuild
```js
// foo.js
var require_foo = __commonJS({
  "foo.js"() {
    console.log("no exports here");
  }
});

// import.js
console.log(Promise.resolve().then(() => __toESM(require_foo())));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/import.js
+++ rolldown	
@@ -1,6 +0,0 @@
-var require_foo = __commonJS({
-    "foo.js"() {
-        console.log("no exports here");
-    }
-});
-console.log(Promise.resolve().then(() => __toESM(require_foo())));

```