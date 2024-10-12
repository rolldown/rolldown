# Diff
## /out/remove-these.js
### esbuild
```js
keepThisButRemoveTheIIFE;
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/remove-these.js
+++ rolldown	
@@ -1,1 +0,0 @@
-keepThisButRemoveTheIIFE;

```
## /out/keep-these.js
### esbuild
```js
undef = void 0;
keepMe();
((x = keepMe()) => {
})();
var someVar;
(([y]) => {
})(someVar);
(({ z }) => {
})(someVar);
var keepThis = stuff();
keepThis();
((_ = keepMe()) => {
})();
var isPure = /* @__PURE__ */ ((x, y) => 123)();
use(isPure);
var isNotPure = ((x = foo, y = bar) => 123)();
use(isNotPure);
(async () => ({ get then() {
  notPure();
} }))();
(async function() {
  return { get then() {
    notPure();
  } };
})();
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/keep-these.js
+++ rolldown	
@@ -1,25 +0,0 @@
-undef = void 0;
-keepMe();
-((x = keepMe()) => {})();
-var someVar;
-(([y]) => {})(someVar);
-(({z}) => {})(someVar);
-var keepThis = stuff();
-keepThis();
-((_ = keepMe()) => {})();
-var isPure = ((x, y) => 123)();
-use(isPure);
-var isNotPure = ((x = foo, y = bar) => 123)();
-use(isNotPure);
-(async () => ({
-    get then() {
-        notPure();
-    }
-}))();
-(async function () {
-    return {
-        get then() {
-            notPure();
-        }
-    };
-})();

```