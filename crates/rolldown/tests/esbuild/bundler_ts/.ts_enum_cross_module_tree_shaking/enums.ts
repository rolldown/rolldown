export enum a_DROP { x = 1 }  // test a dot access
export enum b_DROP { x = 2 }  // test an index access
export enum c_DROP { x = '' } // test a string enum

export enum a_keep { x = false } // false is not inlinable
export enum b_keep { x = foo }   // foo has side effects
export enum c_keep { x = 3 }     // this enum object is captured
export enum d_keep { x = 4 }     // we access "y" on this object
export let e_keep = {}           // non-enum properties should be kept