

# snakenet-migration cli tool

Migration script to migrate data from HydraDX solochain to parachain. 

## Env variables

```bash
    SOURCE_RPC_SERVER = "wss://rpc-01.snakenet.hydradx.io"
    TARGET_RPC_SERVER = "ws://127.0.0.1:9988"
```

### Download

The following command downloads data to `data/storage.json` from the source rpc server.

```bash
    node index.js download -b <block number>
```

Block number is optional. If not provided - the latest block is selected.


### Prepare

The following command takes exported data from `data/storage.json` and transforms it to `data/finalStorage.json`

```bash
    node index.js prepare
```


### Migrate

Following command migrate data to target parachain provided in TARGET_RPC_SERVER.

```bash
    node index.js migrate
```

It takes data from previously downloaded and stored file in `data/storage.json`.

Steps of migration are:
 - load data from `data/storage.json`
 - generate setStorage for each pair
 - retrieve info about max number of call in one batch, max weight of one block
 - split all calls into chunks based on the max calls info
 - submit each batch as sudo


### Validate

Following command performs simple validation between chains after migration.

Currently, it validates account balances and unclaimed claim balance.

```bash
    node index.js validate
```