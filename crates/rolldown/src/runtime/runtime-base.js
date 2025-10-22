
export var __create = Object.create
export var __defProp = Object.defineProperty
export var __name = (target, value) => __defProp(target, "name", { value, configurable: true });
export var __getOwnPropDesc = Object.getOwnPropertyDescriptor
export var __getOwnPropNames = Object.getOwnPropertyNames
export var __getProtoOf = Object.getPrototypeOf
export var __hasOwnProp = Object.prototype.hasOwnProperty
export var __esm = (fn, res) => function () {
  return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res
}
export var __esmMin = (fn, res) => () => (fn && (res = fn(fn = 0)), res)
export var __commonJS = (cb, mod) => function () {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports
}
export var __commonJSMin = (cb, mod) => () => (mod || cb((mod = { exports: {} }).exports, mod), mod.exports)
export var __export = (all) => {
  let target = {}
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true })
  return target;
}
export var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === 'object' || typeof from === 'function')
    for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
      key = keys[i]
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: (k => from[k]).bind(null, key), enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable })
    }
  return to
}
export var __reExport = (target, mod, secondTarget) => (
  __copyProps(target, mod, 'default'),
  secondTarget && __copyProps(secondTarget, mod, 'default')
)
var mergedDefaultExports = /* @__PURE__ */ Symbol()
export var __toESM = (mod, isNodeMode, target) => (
  target = mod != null ? __create(__getProtoOf(mod)) : {},
  __copyProps(
    !mod || !mod.__esModule
      ? __defProp(target, 'default', { value: mod, enumerable: true })
      : isNodeMode
        ? __defProp(target, 'default', {
            value: mergedDefaultExports in mod
              ? mod[mergedDefaultExports]
              : (
                  mod[mergedDefaultExports] =
                    typeof mod.default === 'object' && mod.default
                      ? __copyProps(__copyProps(__create(__getProtoOf(mod.default)), mod.default), mod)
                      : mod
                ),
            enumerable: true
          })
        : target,
    mod)
)
export var __toCommonJS = mod => __copyProps(__defProp({}, '__esModule', { value: true }), mod)
export var __toBinaryNode = base64 => new Uint8Array(Buffer.from(base64, 'base64'))
export var __toBinary = /* @__PURE__ */ (() => {
  var table = new Uint8Array(128)
  for (var i = 0; i < 64; i++) table[i < 26 ? i + 65 : i < 52 ? i + 71 : i < 62 ? i - 4 : i * 4 - 205] = i
  return base64 => {
    var n = base64.length, bytes = new Uint8Array((n - (base64[n - 1] == '=') - (base64[n - 2] == '=')) * 3 / 4 | 0)
    for (var i = 0, j = 0; i < n;) {
      var c0 = table[base64.charCodeAt(i++)], c1 = table[base64.charCodeAt(i++)]
      var c2 = table[base64.charCodeAt(i++)], c3 = table[base64.charCodeAt(i++)]
      bytes[j++] = (c0 << 2) | (c1 >> 4)
      bytes[j++] = (c1 << 4) | (c2 >> 2)
      bytes[j++] = (c2 << 6) | c3
    }
    return bytes
  }
})()

// Rolldown uses this to convert the return value of `import('./some-cjs-module.js')` to a more sensible ESM module namespace.
export var __toDynamicImportESM = (isNodeMode) => (mod) => __toESM(mod.default, isNodeMode)
