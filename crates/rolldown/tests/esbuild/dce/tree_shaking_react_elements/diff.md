# Reason
1. jsx element don't have pure annotation
# Diff
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
let a = /* @__PURE__ */ React.createElement("div", null);
let b = /* @__PURE__ */ React.createElement(Foo, null, a);
let c = /* @__PURE__ */ React.createElement(React.Fragment, null, b);
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
@@ -1,5 +1,8 @@
 function Foo() {}
+var a = React.createElement("div", null);
+var b = React.createElement(Foo, null, a);
+var c = React.createElement(React.Fragment, null, b);
 var d = React.createElement("div", null);
 var e = React.createElement(Foo, null, d);
 var f = React.createElement(React.Fragment, null, e);
 console.log(f);

```