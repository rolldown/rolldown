import { read } from './indirect'
import "./read";

console.log(read)

import('./indirect').then(console.log)
