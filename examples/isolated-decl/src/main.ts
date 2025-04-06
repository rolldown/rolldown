import { Component } from './component';
import { type Num } from './types';
export type Str = string;

export function hello(s: Str): Str {
  return 'hello' + s;
}

export let c: React.JSX.Element = Component;

export let num: Num = 1;
