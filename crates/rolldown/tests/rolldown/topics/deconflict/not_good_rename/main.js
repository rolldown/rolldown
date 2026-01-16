// export * from './other'

// export const Config = (Client) => Client

import { Client } from './lib.js'


const Config = (Client) => {
  console.log(Client)
}

console.log(`Client: `, Client)

Config();
