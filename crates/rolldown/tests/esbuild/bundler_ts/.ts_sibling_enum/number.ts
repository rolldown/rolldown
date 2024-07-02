export enum x { y, yy = y }
export enum x { z = y + 1 }

declare let y: any, z: any
export namespace x { console.log(y, z) }
console.log(x.y, x.z)