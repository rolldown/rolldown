# Reason
1. pure transformation is handled by `oxc-transform`
# Diff
## /out.js
### esbuild
```js
let tests = [
  __spreadValues(__spreadValues({}, a), b),
  __spreadValues({ a, b }, c),
  __spreadProps(__spreadValues({}, a), { b, c }),
  __spreadProps(__spreadValues({ a }, b), { c }),
  __spreadProps(__spreadValues(__spreadValues(__spreadProps(__spreadValues(__spreadValues({ a, b }, c), d), { e, f }), g), h), { i, j })
];
let jsx = [
  /* @__PURE__ */ React.createElement("div", __spreadValues(__spreadValues({}, a), b)),
  /* @__PURE__ */ React.createElement("div", __spreadValues({ a: true, b: true }, c)),
  /* @__PURE__ */ React.createElement("div", __spreadProps(__spreadValues({}, a), { b: true, c: true })),
  /* @__PURE__ */ React.createElement("div", __spreadProps(__spreadValues({ a: true }, b), { c: true })),
  /* @__PURE__ */ React.createElement("div", __spreadProps(__spreadValues(__spreadValues(__spreadProps(__spreadValues(__spreadValues({ a: true, b: true }, c), d), { e: true, f: true }), g), h), { i: true, j: true }))
];
```
### rolldown
```js
import { jsx } from "react/jsx-runtime";

//#region entry.jsx
[
	{
		...a,
		...b
	},
	{
		a,
		b,
		...c
	},
	{
		...a,
		b,
		c
	},
	{
		a,
		...b,
		c
	},
	{
		a,
		b,
		...c,
		...d,
		e,
		f,
		...g,
		...h,
		i,
		j
	}
];
[
	/* @__PURE__ */ jsx("div", {
		...a,
		...b
	}),
	/* @__PURE__ */ jsx("div", {
		a: true,
		b: true,
		...c
	}),
	/* @__PURE__ */ jsx("div", {
		...a,
		b: true,
		c: true
	}),
	/* @__PURE__ */ jsx("div", {
		a: true,
		...b,
		c: true
	}),
	/* @__PURE__ */ jsx("div", {
		a: true,
		b: true,
		...c,
		...d,
		e: true,
		f: true,
		...g,
		...h,
		i: true,
		j: true
	})
];

//#endregion
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,40 +1,55 @@
-let tests = [__spreadValues(__spreadValues({}, a), b), __spreadValues({
+import {jsx} from "react/jsx-runtime";
+[{
+    ...a,
+    ...b
+}, {
     a,
-    b
-}, c), __spreadProps(__spreadValues({}, a), {
     b,
+    ...c
+}, {
+    ...a,
+    b,
     c
-}), __spreadProps(__spreadValues({
-    a
-}, b), {
+}, {
+    a,
+    ...b,
     c
-}), __spreadProps(__spreadValues(__spreadValues(__spreadProps(__spreadValues(__spreadValues({
+}, {
     a,
-    b
-}, c), d), {
+    b,
+    ...c,
+    ...d,
     e,
-    f
-}), g), h), {
+    f,
+    ...g,
+    ...h,
     i,
     j
-})];
-let jsx = [React.createElement("div", __spreadValues(__spreadValues({}, a), b)), React.createElement("div", __spreadValues({
+}];
+[jsx("div", {
+    ...a,
+    ...b
+}), jsx("div", {
     a: true,
-    b: true
-}, c)), React.createElement("div", __spreadProps(__spreadValues({}, a), {
     b: true,
+    ...c
+}), jsx("div", {
+    ...a,
+    b: true,
     c: true
-})), React.createElement("div", __spreadProps(__spreadValues({
-    a: true
-}, b), {
+}), jsx("div", {
+    a: true,
+    ...b,
     c: true
-})), React.createElement("div", __spreadProps(__spreadValues(__spreadValues(__spreadProps(__spreadValues(__spreadValues({
+}), jsx("div", {
     a: true,
-    b: true
-}, c), d), {
+    b: true,
+    ...c,
+    ...d,
     e: true,
-    f: true
-}), g), h), {
+    f: true,
+    ...g,
+    ...h,
     i: true,
     j: true
-}))];
+})];

```