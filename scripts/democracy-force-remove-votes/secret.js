const readline = require("node:readline");
const { Writable } = require("node:stream");

async function promptSecret(prompt) {
  if (!process.stdin.isTTY) throw new Error("submit mode requires an interactive TTY for ACCOUNT_SECRET");
  const silent = new Writable({ write(_chunk, _encoding, callback) { callback(); } });
  const rl = readline.createInterface({ input: process.stdin, output: silent, terminal: true });
  process.stdout.write(prompt);
  try {
    return await new Promise((resolve, reject) => {
      let answered = false;
      rl.on("close", () => {
        if (!answered) reject(new Error("Secret prompt cancelled"));
      });
      rl.question("", (value) => { answered = true; resolve(value); });
    });
  } finally {
    rl.close();
    process.stdout.write("\n");
  }
}

module.exports = { promptSecret };
