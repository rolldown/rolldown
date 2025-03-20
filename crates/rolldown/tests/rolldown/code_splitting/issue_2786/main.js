import shared from "./share";
import sharedJson from "./share.json";

console.log(shared, sharedJson);

import('./share').then(console.log)
import('./share.json').then(console.log)
