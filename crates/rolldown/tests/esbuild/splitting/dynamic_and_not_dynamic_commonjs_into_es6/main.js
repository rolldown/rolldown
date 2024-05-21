import {bar as a} from "./foo.js"
import("./foo.js").then(({default: {bar: b}}) => console.log(a, b))