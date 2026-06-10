import { ApiPromise, WsProvider } from "@polkadot/api";

async function main() {
	console.log("[1] connect");
	const api = await ApiPromise.create({ provider: new WsProvider("ws://127.0.0.1:8013"), noInitWarn: true });
	console.log("  ok, header:", (await api.rpc.chain.getHeader()).number.toNumber());

	console.log("[2] try dev_newBlock via raw provider");
	const provider = (api as any)._rpcCore.provider;
	const result = await provider.send("dev_newBlock", [{ count: 1 }]);
	console.log("  result:", result);

	console.log("[3] header after newBlock:", (await api.rpc.chain.getHeader()).number.toNumber());

	console.log("[4] try dev_setStorage");
	await provider.send("dev_setStorage", [[["0x" + "00".repeat(32), "0x01"]]]);
	console.log("  setStorage ok");

	await api.disconnect();
	console.log("DONE");
}
main().catch(e => { console.error("FAIL:", e); process.exit(1); });
