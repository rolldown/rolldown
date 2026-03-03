## /out.js
### esbuild
```js
// entry.jsx
function Foo() {
}
var d = /* @__PURE__ */ React.createElement("div", null);
var e = /* @__PURE__ */ React.createElement(Foo, null, d);
var f = /* @__PURE__ */ React.createElement(React.Fragment, null, e);
console.log(f);
```
### rolldown
```js
//#region entry.jsx
function Foo() {}
React.Fragment;
let d = /* @__PURE__ */ React.createElement("div", null);
let e = /* @__PURE__ */ React.createElement(Foo, null, d);
let f = /* @__PURE__ */ React.createElement(React.Fragment, null, e);
console.log(f);
//#endregion

```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,5 +1,6 @@
 function Foo() {}
+React.Fragment;
 var d = React.createElement("div", null);
 var e = React.createElement(Foo, null, d);
 var f = React.createElement(React.Fragment, null, e);
 console.log(f);

```