if (false) { // the test case is error case, so we don't need to run it
    import(name).then(pass, fail)
    import(name).then(pass).catch(fail)
    import(name).catch(fail)
}