# Reason
1. rewrite this when it is undefined
# Diff
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
//#region warning2.ts
var foo = void 0 || (void 0).foo;

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/warning2.js
+++ rolldown	warning2.js
@@ -1,2 +1,2 @@
-var foo = (void 0).foo;
+var foo = void 0 || (void 0).foo;
 export {foo};

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
//#region silent1.ts
var foo = void 0 && (void 0).foo;

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/silent1.js
+++ rolldown	silent1.js
@@ -1,2 +1,2 @@
-var foo = void 0;
+var foo = void 0 && (void 0).foo;
 export {foo};

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
//#region silent2.ts
var foo = void 0 && (() => (void 0).foo);

//#endregion
export { foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/silent2.js
+++ rolldown	silent2.js
@@ -1,2 +1,2 @@
-var foo = void 0;
+var foo = void 0 && (() => (void 0).foo);
 export {foo};

```