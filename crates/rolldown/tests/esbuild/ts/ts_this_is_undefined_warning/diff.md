# Diff
## /out/warning1.js
### esbuild
```js
// warning1.ts
var foo = void 0;
export {
  foo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/warning1.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var foo = void 0;
-export {foo};

```
## /out/warning2.js
### esbuild
```js
// warning2.ts
var foo = (void 0).foo;
export {
  foo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/warning2.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var foo = (void 0).foo;
-export {foo};

```
## /out/warning3.js
### esbuild
```js
// warning3.ts
var foo = void 0 ? (void 0).foo : null;
export {
  foo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/warning3.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var foo = void 0 ? (void 0).foo : null;
-export {foo};

```
## /out/silent1.js
### esbuild
```js
// silent1.ts
var foo = void 0;
export {
  foo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/silent1.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var foo = void 0;
-export {foo};

```
## /out/silent2.js
### esbuild
```js
// silent2.ts
var foo = void 0;
export {
  foo
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/silent2.js
+++ rolldown	
@@ -1,2 +0,0 @@
-var foo = void 0;
-export {foo};

```