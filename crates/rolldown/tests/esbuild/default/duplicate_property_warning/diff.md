# Reason
1. lowering jsx
2. not support duplicate property warning
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
import { jsx as _jsx, jsx as _jsx$1 } from "react/jsx-runtime";

//#region outside-node-modules/index.jsx
console.log({
	a: 1,
	a: 2
}, _jsx$1("div", {
	a2: true,
	a2: 3
}));

//#endregion
//#region node_modules/inside-node-modules/index.jsx
console.log({
	c: 1,
	c: 2
}, _jsx("div", {
	c2: true,
	c2: 3
}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/entry.js
+++ rolldown	entry.js
@@ -1,14 +1,15 @@
+import {jsx as _jsx, jsx as _jsx$1} from "react/jsx-runtime";
 console.log({
     a: 1,
     a: 2
-}, React.createElement("div", {
+}, _jsx$1("div", {
     a2: true,
     a2: 3
 }));
 console.log({
     c: 1,
     c: 2
-}, React.createElement("div", {
+}, _jsx("div", {
     c2: true,
     c2: 3
 }));

```