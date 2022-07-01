// This "sandbox" is relatively easy to escape, and does not
// do anything to prevent excessive CPU and RAM usage

const {Worker, isMainThread, workerData, parentPort} = require("worker_threads")
const vm = require("vm")


if (isMainThread) {
  const script = arg(`--script`)
  const repeat = parseInt(arg(`--repeat`) ?? `1`, 10)
  const threads = parseInt(arg(`--threads`) ?? `1`, 10)
  const timeoutMs = parseInt(arg(`--timeout-ms`) ?? `1000`, 10)
  const quiet = process.argv.includes(`--quiet`)

  const run = async (repeat) => {
    for (let i = 0; i < repeat; i++) {
      const start = Date.now()
      const result = await runWorker(script, timeoutMs)
      const end = Date.now()
      if (!quiet) {
        console.warn(`${end - start}ms`)
        console.log(result)
      }
    }
  }
  if (threads > 1) {
    const perThread = Math.ceil(repeat / threads)
    const threadResults = []
    let repeatRemaining = repeat
    while (repeatRemaining > 0) {
      const currentThreadRepeat = Math.min(perThread, repeatRemaining)
      repeatRemaining -= currentThreadRepeat
      threadResults.push(run(currentThreadRepeat))
    }
    Promise.all(threadResults).catch(ex => {
      console.error(ex.stack)
      process.exit(1)
    })
  } else {
    run(repeat).catch(ex => {
      console.error(ex.stack)
      process.exit(1)
    })
  }
} else {
  const result = vm.runInNewContext(workerData.script)
  parentPort.postMessage(result)
}

function arg(name) {
  const idx = process.argv.indexOf(name)
  return idx === -1 ? undefined : process.argv[idx + 1]
}
async function runWorker(script, timeoutMs) {
  const worker = (new Worker(__filename, { workerData: { script }}));
  let result
  let timedOut = false
  const timeout = setTimeout(() => {
    timedOut = true
    worker.terminate()
  }, timeoutMs)
  const exitCode = await new Promise((resolve, reject) => {
    worker.on('message', (msg) => {
      result = msg
    });
    worker.on('error', reject);
    worker.on('exit', resolve);
  })
  clearTimeout(timeout)
  if (timedOut) {
    throw new Error(`Timeout exceeded`)
  }
  if (exitCode !== 0) {
    throw new Error(`Exited with code ${exitCode}`)
  }
  return result
}