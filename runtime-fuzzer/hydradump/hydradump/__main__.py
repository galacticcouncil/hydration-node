from hydradxapi import HydraDX
import tomli_w
import click

RPC = "wss://hydradx-rpc.dwellir.com"

def omnipool():
    chain = HydraDX(RPC)
    chain.connect()
    state = chain.api.omnipool.state()

    def state_as_dict(s):
        return {
            "symbol": s.asset.symbol,
            "asset_id": s.asset.asset_id,
            "reserve": str(s.reserve),
            "hub_reserve": str(s.hub_reserve),
            "shares": str(s.shares),
            "protocol_shares": str(s.protocol_shares),
            "cap": str(s.cap),
            "tradability": str(s.tradability["bits"]),
            "asset_fee": str(s.fees.asset_fee),
            "protocol_fee": str(s.fees.protocol_fee),
        }

    assets =[]
    for s in state.values():
        assets.append(state_as_dict(s))

    return {"asset": assets}

def registry():
    chain = HydraDX(RPC)
    chain.connect()
    state = chain.api.registry.assets()
    assets =[]
    for s in state.values():
        assets.append(s.as_dict())
    return {"asset": assets}


def stableswap():
    chain = HydraDX(RPC)
    chain.connect()
    pools = chain.api.stableswap.pools()

    def transform(d):
        reserves = d["reserves"]
        result = []
        for asset_id,reserve in reserves.items():
            result.append({"asset_id": asset_id, "reserve": str(reserve)})

        d["reserves"] = result
        d["initial_amplification"] = str(d["initial_amplification"])
        d["final_amplification"] = str(d["final_amplification"])
        d["initial_block"] = str(d["initial_block"])
        d["final_block"] = str(d["final_block"])
        return d


    return {"pools": [transform(p.as_dict()) for pool_id, p in pools.items()]}

SUPPORTED_MODULES = {"omnipool": omnipool,
                     "registry": registry,
                     "stableswap": stableswap }

@click.command()
@click.argument("module")
@click.option("-o", "--output", "output", type=click.File('w'))
def hydradump(module, output):
    r = SUPPORTED_MODULES[module]()
    if output:
        output.write(tomli_w.dumps(r))
    else:
        print(r)

hydradump()

