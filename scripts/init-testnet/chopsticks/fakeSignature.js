import { GenericExtrinsic } from '@polkadot/types'
import { signFakeWithApi } from '@acala-network/chopsticks-utils';
import { ApiPromise, WsProvider} from "@polkadot/api";

const CURATOR_ACCOUNT = "7NPoMQbiA6trJKkjB35uk96MeJD4PGWkLQLH7k7hXEkZpiba";

const main = async () => {
    let uri = "ws://127.0.0.1:8000";
    const provider = new WsProvider(uri);
    const api = await ApiPromise.create({ provider });

    //Run the extrinsic in polkadot.js, then in the explorer copy the Extrinsic hash to here
    const tx = await api.tx("36b031ff2405186b9ed12c8ddd811944033bfbaa863679caaf230a61cbc18b2f");
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