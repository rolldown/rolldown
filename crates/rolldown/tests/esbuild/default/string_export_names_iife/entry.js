import { "some import" as someImport } from "./foo"
export { someImport as "some export" }
export * as "all the stuff" from "./foo"