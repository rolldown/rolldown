const svgTable = require('pure-svg-table')
let stats = require('./status.json')

let data = [];

for (var key in stats) {
  data.push([key, stats[key]])
}

let svg = svgTable.generateTable(data, `td {
    padding-top: 4px;
    padding-left: 30px;
    padding-right: 40px;
    padding-bottom: 4px;
    border: 1px solid gray;
    border-collapse: collapse;
}`)
require('fs').writeFileSync('status.svg', svg)

