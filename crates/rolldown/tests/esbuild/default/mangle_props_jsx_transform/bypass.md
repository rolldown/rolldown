# Reason
1. could be done in minifier
# Diff
## /out.js
### esbuild
```js
let Foo = {
  b(props) {
    return /* @__PURE__ */ Foo.a(Foo.d, null, props.c);
  },
  e: "hello, world",
  a(...args) {
    console.log("createElement", ...args);
  },
  d(...args) {
    console.log("Fragment", ...args);
  }
};
export default /* @__PURE__ */ Foo.a(Foo.b, { c: Foo.e });
```
### rolldown
```js
import { Fragment, jsx } from "react/jsx-runtime";

//#region entry.jsx
let Foo = {
	Bar_(props) {
		return /* @__PURE__ */ jsx(Fragment, { children: props.text_ });
	},
	hello_: "hello, world",
	createElement_(...args) {
		console.log("createElement", ...args);
	},
	Fragment_(...args) {
		console.log("Fragment", ...args);
	}
};
var entry_default = /* @__PURE__ */ jsx(Foo.Bar_, { text_: Foo.hello_ });

export { entry_default as default };
```
### diff
```diff
===================================================================
--- esbuild	/out.js
+++ rolldown	entry.js
@@ -1,15 +1,19 @@
-let Foo = {
-    b(props) {
-        return Foo.a(Foo.d, null, props.c);
+import {Fragment, jsx} from "react/jsx-runtime";
+var Foo = {
+    Bar_(props) {
+        return jsx(Fragment, {
+            children: props.text_
+        });
     },
-    e: "hello, world",
-    a(...args) {
+    hello_: "hello, world",
+    createElement_(...args) {
         console.log("createElement", ...args);
     },
-    d(...args) {
+    Fragment_(...args) {
         console.log("Fragment", ...args);
     }
 };
-export default Foo.a(Foo.b, {
-    c: Foo.e
+var entry_default = jsx(Foo.Bar_, {
+    text_: Foo.hello_
 });
+export {entry_default as default};

```