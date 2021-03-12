const { exec } = require("child_process");
const config = require("./" + (process.argv[2] !== undefined ? process.argv[2] : "config.json"));

[config.relaychain.bin, ...config.parachains.map(p => p.bin)].forEach(checkVersion);

function checkVersion(bin) {
  exec(`${bin} --version`, (e, out) => console.log(out.trim()));
}
