# Reason
1. not support enum inline
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
//#region element.tsx
let Foo = /* @__PURE__ */ function(Foo$1) {
	Foo$1["Div"] = "div";
	return Foo$1;
}({});
console.log(/* @__PURE__ */ React.createElement(Foo.Div, null));

//#endregion
export { Foo };
```
### diff
```diff
===================================================================
--- esbuild	/out/element.js
+++ rolldown	element.js
@@ -1,5 +1,6 @@
-export var Foo = (Foo2 => {
-    Foo2["Div"] = "div";
-    return Foo2;
-})(Foo || ({}));
-console.log(React.createElement("div", null));
+var Foo = (function (Foo$1) {
+    Foo$1["Div"] = "div";
+    return Foo$1;
+})({});
+console.log(React.createElement(Foo.Div, null));
+export {Foo};

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
//#region fragment.tsx
let React = /* @__PURE__ */ function(React$1) {
	React$1["Fragment"] = "div";
	return React$1;
}({});
console.log(/* @__PURE__ */ React.createElement(React.Fragment, null, "test"));

//#endregion
export { React };
```
### diff
```diff
===================================================================
--- esbuild	/out/fragment.js
+++ rolldown	fragment.js
@@ -1,5 +1,6 @@
-export var React = (React2 => {
-    React2["Fragment"] = "div";
-    return React2;
-})(React || ({}));
-console.log(React.createElement("div", null, "test"));
+var React = (function (React$1) {
+    React$1["Fragment"] = "div";
+    return React$1;
+})({});
+console.log(React.createElement(React.Fragment, null, "test"));
+export {React};

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
//#region nested-element.tsx
let x;
(function(_x) {
	let y;
	(function(_y) {
		let Foo = /* @__PURE__ */ function(Foo$1) {
			Foo$1["Div"] = "div";
			return Foo$1;
		}({});
		_y.Foo = Foo;
	})(y || (y = _x.y || (_x.y = {})));
})(x || (x = {}));
(function(_x2) {
	let y;
	(function(_y2) {
		console.log(/* @__PURE__ */ React.createElement(x.y.Foo.Div, null));
	})(y || (y = _x2.y || (_x2.y = {})));
})(x || (x = {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/nested-element.js
+++ rolldown	nested-element.js
@@ -1,16 +1,17 @@
 var x;
-(x2 => {
+(function (_x) {
     let y;
-    (y2 => {
-        let Foo;
-        (Foo2 => {
-            Foo2["Div"] = "div";
-        })(Foo = y2.Foo || (y2.Foo = {}));
-    })(y = x2.y || (x2.y = {}));
+    (function (_y) {
+        let Foo = (function (Foo$1) {
+            Foo$1["Div"] = "div";
+            return Foo$1;
+        })({});
+        _y.Foo = Foo;
+    })(y || (y = _x.y || (_x.y = {})));
 })(x || (x = {}));
-(x2 => {
+(function (_x2) {
     let y;
-    (y2 => {
-        console.log(React.createElement("div", null));
-    })(y = x2.y || (x2.y = {}));
+    (function (_y2) {
+        console.log(React.createElement(x.y.Foo.Div, null));
+    })(y || (y = _x2.y || (_x2.y = {})));
 })(x || (x = {}));

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
//#region nested-fragment.tsx
let x;
(function(_x) {
	let y;
	(function(_y) {
		let React$1 = /* @__PURE__ */ function(React$2) {
			React$2["Fragment"] = "div";
			return React$2;
		}({});
		_y.React = React$1;
	})(y || (y = _x.y || (_x.y = {})));
})(x || (x = {}));
(function(_x2) {
	let y;
	(function(_y2) {
		console.log(/* @__PURE__ */ React.createElement(React.Fragment, null, "test"));
	})(y || (y = _x2.y || (_x2.y = {})));
})(x || (x = {}));

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out/nested-fragment.js
+++ rolldown	nested-fragment.js
@@ -1,16 +1,17 @@
 var x;
-(x2 => {
+(function (_x) {
     let y;
-    (y2 => {
-        let React;
-        (React2 => {
-            React2["Fragment"] = "div";
-        })(React = y2.React || (y2.React = {}));
-    })(y = x2.y || (x2.y = {}));
+    (function (_y) {
+        let React$1 = (function (React$2) {
+            React$2["Fragment"] = "div";
+            return React$2;
+        })({});
+        _y.React = React$1;
+    })(y || (y = _x.y || (_x.y = {})));
 })(x || (x = {}));
-(x2 => {
+(function (_x2) {
     let y;
-    (y2 => {
-        console.log(y2.React.createElement("div", null, "test"));
-    })(y = x2.y || (x2.y = {}));
+    (function (_y2) {
+        console.log(React.createElement(React.Fragment, null, "test"));
+    })(y || (y = _x2.y || (_x2.y = {})));
 })(x || (x = {}));

```