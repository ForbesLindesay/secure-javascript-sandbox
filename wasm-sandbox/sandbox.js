export async function evaluate(script, args) {
  let fn
  try {
    fn = new Function(`return (${script})`)()
  } catch {
    throw 'Syntax error in script'
  }
  try {
    const result = await fn(...args.map(arg => JSON.parse(arg)))
    return JSON.stringify(result)
  } catch (error) {
    // error.stack may sometimes be defined but set to an empty string.
    throw `${error.stack || error.message || error}`
  }
}