
//#region shared.js
const shared = "shared";

//#endregion
//#region main.js
const value = `value: ${shared}`;
import("./dynamic.js");

//#endregion
export { value };