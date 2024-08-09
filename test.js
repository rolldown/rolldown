const postfixRE = /[?#].*$/
function cleanUrl(url) {
  return url.replace(postfixRE, '')
}

console.log(cleanUrl('test/test/#test?test=1#test?jfeiajiofeawji=test'))
