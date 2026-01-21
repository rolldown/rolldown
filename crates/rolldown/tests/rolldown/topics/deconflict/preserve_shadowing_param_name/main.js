import { Client } from './lib.js'


// This param shadows the imported `Client`, but should NOT be renamed
// since shadowing is intentional and doesn't cause conflicts at runtime.
const Config = (Client) => {
  console.log(Client)
}

console.log(`Client: `, Client)

Config();
