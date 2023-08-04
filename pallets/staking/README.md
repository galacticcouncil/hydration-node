# Staking Pallet

## Overview

The Staking pallet enables users to stake their HDX with time and governance actions incentivization. Users' rewards are allocated
based on the staked amount and paid based on the number of points they have accumulated. Points consist of `time_points` and `action_points`.
The first user entering staking can claim all the rewards accumulated before they entered. The user's `unpaid_rewards` are returned back to the `pot`
for redistribution to existing stakers.

Staking rewards are collected from trading fees in the omnipool.

#### Terminology

* **time_point** - points for the time a staking position exists. These points are accumulated automatically without the user taking any on-chain actions.
* **action_point** - point for doing various governance actions. These points are accumulated when a user is performing a governance action, e.g. voting
and a staking position exists.
* **unpaid_rewards** - rewards allocated for the user but not paid because they exited early.

## Assumptions

The Staking pallet needs to be initialized before it starts collecting trading fees as rewards or before it can be used by users.
The `initialize_staking` dispatchable is supposed to be used. The staking `pot` account must contain some balance before
the pallet can be initialized. This `pot`'s balance will be used as "non-dustable" balance to prevent account dusting
and won't be distributed to users.

## Interface

### Dispatchable functions

* `initialize_staking` - Staking pallet initialization. Reserve non-dustable balance and create an NFT collection. This must be called first.
* `stake` - Lock the user's HDX into the staking and mint an NFT representing the staking position.
* `increase_stake` - Lock additional HDX into an existing staking position represented by an NFT. Rewards from the old stake are paid to the user and
are locked until the user `claim` or `unstake`. Points accumulated for the old stake are proportionally updated to accommodate the increased stake.
* `claim` - Claim staking rewards for the staking position represented by the NFT. This action is penalized, and unpaid rewards are returned back to
the `pot` for redistribution to users.
* `unstake` - Claim rewards for the staking position, unlock all locked HDX, including HDX locked from increased stake, and destroy the staking position.
