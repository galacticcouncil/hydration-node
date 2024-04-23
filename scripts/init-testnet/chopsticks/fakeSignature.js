import { GenericExtrinsic } from '@polkadot/types'
import { signFakeWithApi } from '@acala-network/chopsticks-utils';
import { ApiPromise, WsProvider} from "@polkadot/api";

const CURATOR_ACCOUNT = "7NzNFJb2pfy2mZzdS2ky8okkXZL52fn2VAvAsc8jigvCtBec";

const main = async () => {
    let uri = "ws://127.0.0.1:8000";
    const provider = new WsProvider(uri);
    const api = await ApiPromise.create({ provider });

    //Run the extrinsic in polkadot.js, then in the explorer copy the Extrinsic hash to here
    const tx = await api.tx("0x690284d43593c715fdd31c61141abd04a99fd6822c8558854ccde39a5684e7a56da27d014c3a8ef5c79f3ba21a3a0e5f31e4ae3ebd5b4d57195b020e14a13acb7518337dc03d74ed9edaac2ffbf7fdb500baf09069926b5bf98362d6e86fccec5942bb8604010400430053420f000000000000642dfcdd070000000000000000000063668916ff2b00000000000000000000040053420f0000000000");

    await signFakeWithApi(api, tx, CURATOR_ACCOUNT);
    await tx.send();

    console.log("DONE BRO");
}

try {
    main();
} catch (e) {
    console.log(e);
    process.exit(1);
}