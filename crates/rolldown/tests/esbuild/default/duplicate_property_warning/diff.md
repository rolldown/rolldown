# Diff
## /out/entry.js
### esbuild
```js
// outside-node-modules/index.jsx
console.log({ a: 1, a: 2 }, /* @__PURE__ */ React.createElement("div", { a2: true, a2: 3 }));

// node_modules/inside-node-modules/index.jsx
console.log({ c: 1, c: 2 }, /* @__PURE__ */ React.createElement("div", { c2: true, c2: 3 }));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	
@@ -1,14 +0,0 @@
-console.log({
-    a: 1,
-    a: 2
-}, React.createElement("div", {
-    a2: true,
-    a2: 3
-}));
-console.log({
-    c: 1,
-    c: 2
-}, React.createElement("div", {
-    c2: true,
-    c2: 3
-}));

```