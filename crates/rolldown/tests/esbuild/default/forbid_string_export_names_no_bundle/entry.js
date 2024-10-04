let ok = true
export { ok as "ok", ok as "not ok" }
export { "same name" } from "./foo"
export { "name 1" as "name 2" } from "./foo"
export * as "name space" from "./foo"