import 'a_nowrap'

import { b } from 'b_nowrap'
b()

export * from 'c_nowrap'

import * as d from 'd_WRAP'
x = d.x

import e from 'e_WRAP'
e()

import { default as f } from 'f_WRAP'
f()

import { __esModule as g } from 'g_WRAP'
g()

import * as h from 'h_WRAP'
x = h

import * as i from 'i_WRAP'
i.x()

import * as j from 'j_WRAP'
j.x` + "``" + `

x = import("k_WRAP")