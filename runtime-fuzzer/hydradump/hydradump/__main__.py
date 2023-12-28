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

SUPPORTED_MODULES = {"omnipool": omnipool}

@click.command()
@click.argument("module")
@click.option("-o", "--output", "output", type=click.File('w'))
def hydradump(module, output):
  r = SUPPORTED_MODULES[module]()
  if output:
    output.write(tomli_w.dumps(r))

hydradump()

