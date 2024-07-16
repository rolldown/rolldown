import http from 'node:http'
import fsExtra from 'fs-extra'

// Using remapping benchmark
if (fsExtra.existsSync('./tmp/bench/antd')) {
  console.log('[skip] setup antd already')
} else {
  console.log('Setup `antd` in tmp/bench')
  http.get('http://cdn.jsdelivr.net/npm/antd@5.12.5/dist/antd.js', (res) => {
    fsExtra.ensureDirSync('./tmp/bench/antd')
    const writeStream = fsExtra.createWriteStream('./tmp/bench/antd/antd.js')
    res.pipe(writeStream)
  })
}
