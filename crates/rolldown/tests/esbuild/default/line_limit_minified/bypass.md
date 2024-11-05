# Reason
1. Line Limit is out of rolldown scope, sicne rolldown use `oxc` to convert ast to string
# Diff
## /out/script.js
### esbuild
```js
export const SignUpForm=props=>{
return React.createElement("p",{
class:"signup"},React.createElement(
"label",null,"Username: ",React.
createElement("input",{class:"us\
ername",type:"text"})),React.createElement(
"label",null,"Password: ",React.
createElement("input",{class:"pa\
ssword",type:"password"})),React.
createElement("div",{class:"prim\
ary disabled"},props.buttonText),
React.createElement("small",null,
"By signing up, you are agreeing\
 to our ",React.createElement("a",
{href:"/tos/"},"terms of service"),
"."))};
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/script.js
+++ rolldown	
@@ -1,19 +0,0 @@
-export const SignUpForm = props => {
-    return React.createElement("p", {
-        class: "signup"
-    }, React.createElement("label", null, "Username: ", React.createElement("input", {
-        class: "us\
-ername",
-        type: "text"
-    })), React.createElement("label", null, "Password: ", React.createElement("input", {
-        class: "pa\
-ssword",
-        type: "password"
-    })), React.createElement("div", {
-        class: "prim\
-ary disabled"
-    }, props.buttonText), React.createElement("small", null, "By signing up, you are agreeing\
- to our ", React.createElement("a", {
-        href: "/tos/"
-    }, "terms of service"), "."));
-};

```
## /out/style.css
### esbuild
```js
body.light-mode.new-user-segment:not(.logged-in)
.signup,body.light-mode.new-user-segment:not(.logged-in)
.login{font:10px/12px "Font 1","\
Font 2","Font 3","Font 4",sans-serif;
user-select:none;color:var(--fg,
rgba(11, 22, 33, 0.5));background:url("\
data:image/svg+xml;base64,PHN2Zy\
B3aWR0aD0iMjAwIiBoZWlnaHQ9IjIwMC\
IgeG1sbnM9Imh0dHA6Ly93d3cudzMub3\
JnLzIwMDAvc3ZnIj4KICA8Y2lyY2xlIG\
N4PSIxMDAiIGN5PSIxMDAiIHI9IjEwMC\
IgZmlsbD0iI0ZGQ0YwMCIvPgogIDxwYX\
RoIGQ9Ik00Ny41IDUyLjVMOTUgMTAwbC\
00Ny41IDQ3LjVtNjAtOTVMMTU1IDEwMG\
wtNDcuNSA0Ny41IiBmaWxsPSJub25lIi\
BzdHJva2U9IiMxOTE5MTkiIHN0cm9rZS\
13aWR0aD0iMjQiLz4KPC9zdmc+Cg==")}
```
### rolldown
```js

```
### diff
```diff
===================================================================
--- esbuild	/out/style.css
+++ rolldown	
@@ -1,17 +0,0 @@
-body.light-mode.new-user-segment:not(.logged-in)
-.signup,body.light-mode.new-user-segment:not(.logged-in)
-.login{font:10px/12px "Font 1","\
-Font 2","Font 3","Font 4",sans-serif;
-user-select:none;color:var(--fg,
-rgba(11, 22, 33, 0.5));background:url("\
-data:image/svg+xml;base64,PHN2Zy\
-B3aWR0aD0iMjAwIiBoZWlnaHQ9IjIwMC\
-IgeG1sbnM9Imh0dHA6Ly93d3cudzMub3\
-JnLzIwMDAvc3ZnIj4KICA8Y2lyY2xlIG\
-N4PSIxMDAiIGN5PSIxMDAiIHI9IjEwMC\
-IgZmlsbD0iI0ZGQ0YwMCIvPgogIDxwYX\
-RoIGQ9Ik00Ny41IDUyLjVMOTUgMTAwbC\
-00Ny41IDQ3LjVtNjAtOTVMMTU1IDEwMG\
-wtNDcuNSA0Ny41IiBmaWxsPSJub25lIi\
-BzdHJva2U9IiMxOTE5MTkiIHN0cm9rZS\
-13aWR0aD0iMjQiLz4KPC9zdmc+Cg==")}
\ No newline at end of file

```