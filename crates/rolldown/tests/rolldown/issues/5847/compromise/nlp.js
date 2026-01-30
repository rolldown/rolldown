const nlp = function () {
  console.log(666)
}
nlp.plugin = function () {
  return this
}
export default nlp