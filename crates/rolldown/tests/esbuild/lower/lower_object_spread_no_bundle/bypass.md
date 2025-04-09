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
import { jsx as jsx$1 } from "react/jsx-runtime";

//#region entry.jsx
let tests = [
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
let jsx$2 = [
	/* @__PURE__ */ jsx$1("div", {
		...a,
		...b
	}),
	/* @__PURE__ */ jsx$1("div", {
		a: true,
		b: true,
		...c
	}),
	/* @__PURE__ */ jsx$1("div", {
		...a,
		b: true,
		c: true
	}),
	/* @__PURE__ */ jsx$1("div", {
		a: true,
		...b,
		c: true
	}),
	/* @__PURE__ */ jsx$1("div", {
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
+import {jsx as jsx$1} from "react/jsx-runtime";
+var tests = [{
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
+var jsx$2 = [jsx$1("div", {
+    ...a,
+    ...b
+}), jsx$1("div", {
     a: true,
-    b: true
-}, c)), React.createElement("div", __spreadProps(__spreadValues({}, a), {
     b: true,
+    ...c
+}), jsx$1("div", {
+    ...a,
+    b: true,
     c: true
-})), React.createElement("div", __spreadProps(__spreadValues({
-    a: true
-}, b), {
+}), jsx$1("div", {
+    a: true,
+    ...b,
     c: true
-})), React.createElement("div", __spreadProps(__spreadValues(__spreadValues(__spreadProps(__spreadValues(__spreadValues({
+}), jsx$1("div", {
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