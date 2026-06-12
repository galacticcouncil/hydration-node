#!/usr/bin/env node

let ApiPromise;
let WsProvider;
let Keyring;
let cryptoWaitReady;

function loadPackage(packageName) {
	try {
		return require(packageName);
	} catch (error) {
		const paths = (process.env.PATH || "").split(require("path").delimiter);
		for (const entry of paths) {
			if (!entry.endsWith(`${require("path").sep}node_modules${require("path").sep}.bin`)) {
				continue;
			}

			const nodeModules = require("path").dirname(entry);
			try {
				return require(require("path").join(nodeModules, packageName));
			} catch (_) {
				// Try the next npm exec temp directory.
			}
		}

		throw error;
	}
}

try {
	({ ApiPromise, WsProvider, Keyring } = loadPackage("@polkadot/api"));
	({ cryptoWaitReady } = loadPackage("@polkadot/util-crypto"));
} catch (error) {
	console.error(
		"Missing JS deps. Run without installing into the repo via:\n" +
			"npm exec --yes --package=@polkadot/api --package=@polkadot/util-crypto -- node scripts/assign_cores.js",
	);
	process.exit(1);
}

function parseArgs(argv) {
	const defaults = {
		ws: "ws://127.0.0.1:9945",
		suri: "//Alice",
		paraId: 2032,
		cores: [0, 1, 2],
		begin: 0,
		finalized: true,
	};

	for (let i = 0; i < argv.length; i += 1) {
		const arg = argv[i];
		if (arg === "--ws") {
			defaults.ws = argv[++i];
		} else if (arg === "--suri") {
			defaults.suri = argv[++i];
		} else if (arg === "--para") {
			defaults.paraId = Number(argv[++i]);
		} else if (arg === "--cores") {
			defaults.cores = argv[++i]
				.split(",")
				.filter(Boolean)
				.map((value) => Number(value.trim()));
		} else if (arg === "--begin") {
			defaults.begin = Number(argv[++i]);
		} else if (arg === "--in-block") {
			defaults.finalized = false;
		} else if (arg === "--help" || arg === "-h") {
			printHelp();
			process.exit(0);
		} else {
			throw new Error(`Unknown argument: ${arg}`);
		}
	}

	if (!defaults.cores.length || defaults.cores.some((core) => Number.isNaN(core))) {
		throw new Error("Expected --cores to contain a comma-separated list of integers");
	}

	if (Number.isNaN(defaults.paraId)) {
		throw new Error("Expected --para to be an integer");
	}

	if (Number.isNaN(defaults.begin)) {
		throw new Error("Expected --begin to be an integer");
	}

	return defaults;
}

function printHelp() {
	console.log(`Assign relay-chain cores to a parachain on a local Zombienet relay node.

Usage:
  node scripts/assign_cores.js [options]

Options:
  --ws <url>        Relay-chain websocket endpoint (default: ws://127.0.0.1:9945)
  --suri <suri>     Signing account SURI (default: //Alice)
  --para <id>       Parachain id to assign cores to (default: 2032)
  --cores <list>    Comma-separated core indexes (default: 0,1,2)
  --begin <block>   Relay block number to start assignment from (default: 0)
  --in-block        Exit once included in a block instead of waiting for finalization
  --help, -h        Show this message
`);
}

async function main() {
	const { ws, suri, paraId, cores, begin, finalized } = parseArgs(process.argv.slice(2));

	await cryptoWaitReady();

	const provider = new WsProvider(ws);
	const api = await ApiPromise.create({ provider });
	const keyring = new Keyring({ type: "sr25519" });
	const signer = keyring.addFromUri(suri);

	const calls = cores.map((core) =>
		api.tx.coretime.assignCore(
			core,
			begin,
			[[{ Task: paraId }, 57600]],
			null,
		),
	);

	const tx = api.tx.sudo.sudo(api.tx.utility.batch(calls));

	console.log(
		`Submitting assign_core for para ${paraId} on cores [${cores.join(", ")}] via ${ws} as ${signer.address}`,
	);

	await new Promise(async (resolve, reject) => {
		let unsub = null;

		try {
			unsub = await tx.signAndSend(signer, ({ status, dispatchError, events }) => {
				if (dispatchError) {
					if (dispatchError.isModule) {
						const decoded = api.registry.findMetaError(dispatchError.asModule);
						reject(
							new Error(
								`${decoded.section}.${decoded.name}: ${decoded.docs.join(" ")}`,
							),
						);
					} else {
						reject(new Error(dispatchError.toString()));
					}
					return;
				}

				if (status.isInBlock) {
					console.log(`Included at ${status.asInBlock.toHex()}`);
					if (!finalized) {
						if (unsub) {
							unsub();
						}
						resolve();
					}
				}

				if (status.isFinalized) {
					console.log(`Finalized at ${status.asFinalized.toHex()}`);
					for (const { event } of events) {
						console.log(`Event: ${event.section}.${event.method}`);
					}
					if (unsub) {
						unsub();
					}
					resolve();
				}
			});
		} catch (error) {
			if (unsub) {
				unsub();
			}
			reject(error);
		}
	});

	await api.disconnect();
}

main().catch((error) => {
	console.error(error.message || error);
	process.exit(1);
});
