# Script to determine the number of trades to expect in a block.
# 
# Note: Rerun if any of `MAX_NORMAL_WEIGHT`, xyk weight (or other trade weights) or oracle weights
# change. 

WEIGHT_PER_SECOND = 1_000_000_000_000

ROCKS_DB_READ = 25_000_000
ROCKS_DB_WRITE = 100_000_000

MAX_BLOCK_WEIGHT = WEIGHT_PER_SECOND / 2

MAX_NORMAL_WEIGHT = MAX_BLOCK_WEIGHT * 0.75

BASE_EXTRINSIC_WEIGHT = 86_298_000

def on_finalize_no_entry():
    return 2_373_000 + ROCKS_DB_READ

def on_finalize_multiple_tokens(b):
    return 48_128_000 * b + ROCKS_DB_READ + ROCKS_DB_READ * 4 * b + ROCKS_DB_WRITE + ROCKS_DB_WRITE * 4 * b

def on_trade_multiple_tokens(b):
    return 20_775_000 + 465_000 * b + ROCKS_DB_READ + ROCKS_DB_WRITE

def on_liquidity_changed_multiple_tokens(b):
    return 20_467_000 + 467_000 * b + ROCKS_DB_READ + ROCKS_DB_WRITE

def xyk_buy():
	return 111_306_000 + ROCKS_DB_READ * 11 + ROCKS_DB_WRITE * 5

def on_trade_weight(num_tokens):
    return on_trade_multiple_tokens(num_tokens) + (on_finalize_multiple_tokens(num_tokens) - on_finalize_no_entry()) / num_tokens

def on_liquidity_changed_weight(num_tokens):
    return on_liquidity_changed_multiple_tokens(num_tokens) + (on_finalize_multiple_tokens(num_tokens) - on_finalize_no_entry()) / num_tokens

def number_of_buys(callback_weight):
    return MAX_NORMAL_WEIGHT / (BASE_EXTRINSIC_WEIGHT + xyk_buy() + callback_weight)

print("number of bare xyk buys:")
num_bare_buys = number_of_buys(0)
print(num_bare_buys)
print("on_trade weight for that many buys:")
print(on_trade_weight(num_bare_buys))
print("bare xyk_buy weight (without callback weight):")
print(BASE_EXTRINSIC_WEIGHT + xyk_buy())

# iteratively determine the number of trades that can fit in a block, reaches exiquilibrium quickly
num_buys = num_bare_buys
for i in range(0, 50):
    num_buys = number_of_buys(on_trade_weight(num_buys))

print("")
print("on_trade weight after 50 iterations:")
print(on_trade_weight(num_buys))
print("number of trades fitting into one block after 50 iterations of on_trade:")
print(num_buys)

num_buys = num_bare_buys
for i in range(0, 50):
    num_buys = number_of_buys(on_liquidity_changed_weight(num_buys))

print("")
print("on_liquidity_changed weight after 50 iterations:")
print(on_liquidity_changed_weight(num_buys))
print("number of trades fitting into one block after 50 iterations of on_trade:")
print(num_buys)
