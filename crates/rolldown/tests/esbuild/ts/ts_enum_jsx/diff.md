# Diff
## /out/element.js
### esbuild
```js
export var Foo = /* @__PURE__ */ ((Foo2) => {
  Foo2["Div"] = "div";
  return Foo2;
})(Foo || {});
console.log(/* @__PURE__ */ React.createElement("div" /* Div */, null));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/element.js
+++ rolldown	
@@ -1,5 +0,0 @@
-export var Foo = (Foo2 => {
-    Foo2["Div"] = "div";
-    return Foo2;
-})(Foo || ({}));
-console.log(React.createElement("div", null));

```
## /out/fragment.js
### esbuild
```js
export var React = /* @__PURE__ */ ((React2) => {
  React2["Fragment"] = "div";
  return React2;
})(React || {});
console.log(/* @__PURE__ */ React.createElement("div" /* Fragment */, null, "test"));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	
@@ -1,5 +0,0 @@
-export var React = (React2 => {
-    React2["Fragment"] = "div";
-    return React2;
-})(React || ({}));
-console.log(React.createElement("div", null, "test"));

```
## /out/nested-element.js
### esbuild
```js
var x;
((x2) => {
  let y;
  ((y2) => {
    let Foo;
    ((Foo2) => {
      Foo2["Div"] = "div";
    })(Foo = y2.Foo || (y2.Foo = {}));
  })(y = x2.y || (x2.y = {}));
})(x || (x = {}));
((x2) => {
  let y;
  ((y2) => {
    console.log(/* @__PURE__ */ React.createElement("div" /* Div */, null));
  })(y = x2.y || (x2.y = {}));
})(x || (x = {}));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-element.js
+++ rolldown	
@@ -1,16 +0,0 @@
-var x;
-(x2 => {
-    let y;
-    (y2 => {
-        let Foo;
-        (Foo2 => {
-            Foo2["Div"] = "div";
-        })(Foo = y2.Foo || (y2.Foo = {}));
-    })(y = x2.y || (x2.y = {}));
-})(x || (x = {}));
-(x2 => {
-    let y;
-    (y2 => {
-        console.log(React.createElement("div", null));
-    })(y = x2.y || (x2.y = {}));
-})(x || (x = {}));

```
## /out/nested-fragment.js
### esbuild
```js
var x;
((x2) => {
  let y;
  ((y2) => {
    let React;
    ((React2) => {
      React2["Fragment"] = "div";
    })(React = y2.React || (y2.React = {}));
  })(y = x2.y || (x2.y = {}));
})(x || (x = {}));
((x2) => {
  let y;
  ((y2) => {
    console.log(/* @__PURE__ */ y2.React.createElement("div" /* Fragment */, null, "test"));
  })(y = x2.y || (x2.y = {}));
})(x || (x = {}));
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/nested-fragment.js
+++ rolldown	
@@ -1,16 +0,0 @@
-var x;
-(x2 => {
-    let y;
-    (y2 => {
-        let React;
-        (React2 => {
-            React2["Fragment"] = "div";
-        })(React = y2.React || (y2.React = {}));
-    })(y = x2.y || (x2.y = {}));
-})(x || (x = {}));
-(x2 => {
-    let y;
-    (y2 => {
-        console.log(y2.React.createElement("div", null, "test"));
-    })(y = x2.y || (x2.y = {}));
-})(x || (x = {}));

```