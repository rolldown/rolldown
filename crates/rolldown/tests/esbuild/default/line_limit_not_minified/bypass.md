# Reason
1. Line Limit is out of rolldown scope, sicne rolldown use `oxc` to convert ast to string
# Diff
## /out/x-TZ25B4WH.file
### esbuild
```js
...file...
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/x-TZ25B4WH.file
+++ rolldown	
@@ -1,1 +0,0 @@
-...file...
\ No newline at end of file

```
## /out/x-UF3O47Y3.copy
### esbuild
```js
...copy...
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/x-UF3O47Y3.copy
+++ rolldown	
@@ -1,1 +0,0 @@
-...copy...
\ No newline at end of file

```
## /out/script.js
### esbuild
```js
// x.file
var x_default = "./x-TZ25B4WH.file";

// script.jsx
import copyURL from "./x-UF3O47Y3.copy";

// x.data
var x_default2 = "data:text/plai\
n;charset=utf-8,...lots of long \
data...lots of long data...";

// script.jsx
var SignUpForm = (props) => {
  return /* @__PURE__ */ React.createElement(
  "p", { class: "signup" }, /* @__PURE__ */ React.
  createElement("label", null, "\
Username: ", /* @__PURE__ */ React.
  createElement("input", { class: "\
username", type: "text" })), /* @__PURE__ */ React.
  createElement("label", null, "\
Password: ", /* @__PURE__ */ React.
  createElement("input", { class: "\
password", type: "password" })),
  /* @__PURE__ */ React.createElement(
  "div", { class: "primary disab\
led" }, props.buttonText), /* @__PURE__ */ React.
  createElement("small", null, "\
By signing up, you are agreeing \
to our ", /* @__PURE__ */ React.
  createElement("a", { href: "/t\
os/" }, "terms of service"), "."),
  /* @__PURE__ */ React.createElement(
  "img", { src: x_default }), /* @__PURE__ */ React.
  createElement("img", { src: copyURL }),
  /* @__PURE__ */ React.createElement(
  "img", { src: x_default2 }));
};
export {
  SignUpForm
};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/script.js
+++ rolldown	
@@ -1,35 +0,0 @@
-var x_default = "./x-TZ25B4WH.file";
-import copyURL from "./x-UF3O47Y3.copy";
-var x_default2 = "data:text/plai\
-n;charset=utf-8,...lots of long \
-data...lots of long data...";
-var SignUpForm = props => {
-    return React.createElement("p", {
-        class: "signup"
-    }, React.createElement("label", null, "\
-Username: ", React.createElement("input", {
-        class: "\
-username",
-        type: "text"
-    })), React.createElement("label", null, "\
-Password: ", React.createElement("input", {
-        class: "\
-password",
-        type: "password"
-    })), React.createElement("div", {
-        class: "primary disab\
-led"
-    }, props.buttonText), React.createElement("small", null, "\
-By signing up, you are agreeing \
-to our ", React.createElement("a", {
-        href: "/t\
-os/"
-    }, "terms of service"), "."), React.createElement("img", {
-        src: x_default
-    }), React.createElement("img", {
-        src: copyURL
-    }), React.createElement("img", {
-        src: x_default2
-    }));
-};
-export {SignUpForm};

```
## /out/style.css
### esbuild
```js
/* style.css */
body.light-mode.new-user-segment:not(.logged-in)
.signup,
body.light-mode.new-user-segment:not(.logged-in)
.login {
  font:
    10px/12px "Font 1",
    "Font 2",
    "Font 3",
    "Font 4",
    sans-serif;
  user-select: none;
  color: var(--fg, rgba(11, 22, 33,
    0.5));
  background: url("data:image/sv\
g+xml;base64,PHN2ZyB3aWR0aD0iMjA\
wIiBoZWlnaHQ9IjIwMCIgeG1sbnM9Imh\
0dHA6Ly93d3cudzMub3JnLzIwMDAvc3Z\
nIj4KICA8Y2lyY2xlIGN4PSIxMDAiIGN\
5PSIxMDAiIHI9IjEwMCIgZmlsbD0iI0Z\
GQ0YwMCIvPgogIDxwYXRoIGQ9Ik00Ny4\
1IDUyLjVMOTUgMTAwbC00Ny41IDQ3LjV\
tNjAtOTVMMTU1IDEwMGwtNDcuNSA0Ny4\
1IiBmaWxsPSJub25lIiBzdHJva2U9IiM\
xOTE5MTkiIHN0cm9rZS13aWR0aD0iMjQ\
iLz4KPC9zdmc+Cg==");
  cursor: url("./x-TZ25B4WH.file");
  cursor: url("./x-UF3O47Y3.copy");
  cursor: url("data:text/plain;c\
harset=utf-8,...lots of long dat\
a...lots of long data...");
}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/style.css
+++ rolldown	
@@ -1,32 +0,0 @@
-/* style.css */
-body.light-mode.new-user-segment:not(.logged-in)
-.signup,
-body.light-mode.new-user-segment:not(.logged-in)
-.login {
-  font:
-    10px/12px "Font 1",
-    "Font 2",
-    "Font 3",
-    "Font 4",
-    sans-serif;
-  user-select: none;
-  color: var(--fg, rgba(11, 22, 33,
-    0.5));
-  background: url("data:image/sv\
-g+xml;base64,PHN2ZyB3aWR0aD0iMjA\
-wIiBoZWlnaHQ9IjIwMCIgeG1sbnM9Imh\
-0dHA6Ly93d3cudzMub3JnLzIwMDAvc3Z\
-nIj4KICA8Y2lyY2xlIGN4PSIxMDAiIGN\
-5PSIxMDAiIHI9IjEwMCIgZmlsbD0iI0Z\
-GQ0YwMCIvPgogIDxwYXRoIGQ9Ik00Ny4\
-1IDUyLjVMOTUgMTAwbC00Ny41IDQ3LjV\
-tNjAtOTVMMTU1IDEwMGwtNDcuNSA0Ny4\
-1IiBmaWxsPSJub25lIiBzdHJva2U9IiM\
-xOTE5MTkiIHN0cm9rZS13aWR0aD0iMjQ\
-iLz4KPC9zdmc+Cg==");
-  cursor: url("./x-TZ25B4WH.file");
-  cursor: url("./x-UF3O47Y3.copy");
-  cursor: url("data:text/plain;c\
-harset=utf-8,...lots of long dat\
-a...lots of long data...");
-}
\ No newline at end of file

```