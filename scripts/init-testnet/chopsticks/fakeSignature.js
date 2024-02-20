import { GenericExtrinsic } from '@polkadot/types'
import { signFakeWithApi } from '@acala-network/chopsticks-utils';
import { ApiPromise, WsProvider} from "@polkadot/api";

const CURATOR_ACCOUNT = "7KejvRw4GZvVjFQEDefAsBRd9iaTjVeUWczB44Mgu8Bue8JW";

const main = async () => {
    let uri = "ws://127.0.0.1:8000";
    const provider = new WsProvider(uri);
    const api = await ApiPromise.create({ provider });

    const tx = await api.tx("0xa102845ae602686c9573bff2ef0483bda7a9589bc9eb7a2e14f91296367c5785394b2701dc7f3ddd4e87e63d69df30f91253211ebd2ca90b3269c883f976e9d05684857cde29541721f6ba400efeda25086098fde8c7d525fef11435a9d04255f2e68d8df40165070043010a00000000000000c83200000000000000000000000000000098f73e5d0100000000000000000000080300000000640000000264000000640000000a000000");

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