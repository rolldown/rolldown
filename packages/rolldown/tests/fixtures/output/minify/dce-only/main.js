/*! legal comment is kept */

import(/* @vite-ignore annotation comment is kept */ 'node:module')

export function foo() {
  const varNameIsNotMangled = window.something
  return varNameIsNotMangled + varNameIsNotMangled
}
