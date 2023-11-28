
var __create = Object.create
var __defProp = Object.defineProperty
var __getOwnPropDesc = Object.getOwnPropertyDescriptor
var __getOwnPropNames = Object.getOwnPropertyNames
var __getProtoOf = Object.getPrototypeOf
var __hasOwnProp = Object.prototype.hasOwnProperty
var __esm = (fn, res) => function () {
  return fn && (res = (0, fn[__getOwnPropNames(fn)[0]])(fn = 0)), res
}
var __esmMin = (fn, res) => () => (fn && (res = fn(fn = 0)), res)
var __commonJS = (cb, mod) => function () {
  return mod || (0, cb[__getOwnPropNames(cb)[0]])((mod = { exports: {} }).exports, mod), mod.exports
}
var __commonJSMin = (cb, mod) => () => (mod || cb((mod = { exports: {} }).exports, mod), mod.exports)
var __export = (target, all) => {
  for (var name in all)
    __defProp(target, name, { get: all[name], enumerable: true })
}
var __copyProps = (to, from, except, desc) => {
  if (from && typeof from === 'object' || typeof from === 'function')
    for (var keys = __getOwnPropNames(from), i = 0, n = keys.length, key; i < n; i++) {
      key = keys[i]
      if (!__hasOwnProp.call(to, key) && key !== except)
        __defProp(to, key, { get: (k => from[k]).bind(null, key), enumerable: !(desc = __getOwnPropDesc(from, key)) || desc.enumerable })
    }
  return to
}
var __reExport = (target, mod, secondTarget) => (
  __copyProps(target, mod, 'default'),
  secondTarget && __copyProps(secondTarget, mod, 'default')
)
var __toESM = (mod, isNodeMode, target) => (
  target = mod != null ? __create(__getProtoOf(mod)) : {},
  __copyProps(
    isNodeMode || !mod || !mod.__esModule
      ? __defProp(target, 'default', { value: mod, enumerable: true })
      : target,
    mod)
)
var __toCommonJS = mod => __copyProps(__defProp({}, '__esModule', { value: true }), mod)
