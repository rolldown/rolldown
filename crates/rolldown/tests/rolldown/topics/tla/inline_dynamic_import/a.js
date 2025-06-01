import { getEnv } from './b.js'

export const buildDevConfig = async () => {
  return await getEnv()
}
