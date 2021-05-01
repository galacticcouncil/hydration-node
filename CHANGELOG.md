# v5.0.0 (Tue Apr 20 2021)

#### 💥 Breaking Change

- feat!: Snakenet gen2 - governance, epoch times and treasury [#188](https://github.com/galacticcouncil/HydraDX-node/pull/188) ([@Roznovjak](https://github.com/Roznovjak) [@green-jay](https://github.com/green-jay) [@jak-pan](https://github.com/jak-pan) [@lumir-mrkva](https://github.com/lumir-mrkva) [@martinfridrich](https://github.com/martinfridrich) [@enthusiastmartin](https://github.com/enthusiastmartin))
- fix(runtime)!: add tx fee multiplier [#208](https://github.com/galacticcouncil/HydraDX-node/pull/208) ([@enthusiastmartin](https://github.com/enthusiastmartin))
- feat!: genesis history pallet [#202](https://github.com/galacticcouncil/HydraDX-node/pull/202) ([@lumir-mrkva](https://github.com/lumir-mrkva))
- fix!:  set DOLLARS constant to correct number [#200](https://github.com/galacticcouncil/HydraDX-node/pull/200) ([@jak-pan](https://github.com/jak-pan))
- fix!: offence is reported but slashing is not applied [#199](https://github.com/galacticcouncil/HydraDX-node/pull/199) ([@martinfridrich](https://github.com/martinfridrich) [@jak-pan](https://github.com/jak-pan))
- feat!: governance [#183](https://github.com/galacticcouncil/HydraDX-node/pull/183) ([@Roznovjak](https://github.com/Roznovjak) [@green-jay](https://github.com/green-jay) [@jak-pan](https://github.com/jak-pan))
- feat!: change epoch to 4 hours [#187](https://github.com/galacticcouncil/HydraDX-node/pull/187) ([@jak-pan](https://github.com/jak-pan))
- feat!: setup technical committee [#174](https://github.com/galacticcouncil/HydraDX-node/pull/174) ([@Roznovjak](https://github.com/Roznovjak) [@green-jay](https://github.com/green-jay))

#### 🚀 Enhancement

- feat: version consistent with tagged release [#194](https://github.com/galacticcouncil/HydraDX-node/pull/194) ([@lumir-mrkva](https://github.com/lumir-mrkva) [@jak-pan](https://github.com/jak-pan))

#### Refactoring

- refactor(genesis-history): derived default genesis chain [#211](https://github.com/galacticcouncil/HydraDX-node/pull/211) ([@lumir-mrkva](https://github.com/lumir-mrkva))

#### Authors: 6

- [@lumir-mrkva](https://github.com/lumir-mrkva)
- Jakub Pánik ([@jak-pan](https://github.com/jak-pan))
- Jindrich Zeleny ([@green-jay](https://github.com/green-jay))
- martin fridrich ([@martinfridrich](https://github.com/martinfridrich))
- Martin Hloska ([@enthusiastmartin](https://github.com/enthusiastmartin))
- Richard Roznovjak ([@Roznovjak](https://github.com/Roznovjak))

---

# v4.0.1 (Tue Mar 30 2021)

#### 🐛 Bug Fix

- fix: telemetry [#190](https://github.com/galacticcouncil/HydraDX-node/pull/190) ([@lumir-mrkva](https://github.com/lumir-mrkva))

#### Authors: 1

- [@lumir-mrkva](https://github.com/lumir-mrkva)

---

# v4.0.0 (Mon Mar 29 2021)

#### 💥 Breaking Change

- feat!: disable slashing [#184](https://github.com/galacticcouncil/HydraDX-node/pull/184) ([@martinfridrich](https://github.com/martinfridrich) [@jak-pan](https://github.com/jak-pan))
- feat!: add identity pallet [#163](https://github.com/galacticcouncil/HydraDX-node/pull/163) ([@Roznovjak](https://github.com/Roznovjak) [@lumir-mrkva](https://github.com/lumir-mrkva) [@jak-pan](https://github.com/jak-pan))
- feat!: set reward curve params for inc. testnet [#173](https://github.com/galacticcouncil/HydraDX-node/pull/173) ([@green-jay](https://github.com/green-jay))

#### 🐛 Bug Fix

- bug(perf-check): fix bench-wizard install/upgrade [#179](https://github.com/galacticcouncil/HydraDX-node/pull/179) ([@enthusiastmartin](https://github.com/enthusiastmartin))

#### Other improvements

- chore(perf-check): install bench wizard as part of the check perf script [#170](https://github.com/galacticcouncil/HydraDX-node/pull/170) ([@enthusiastmartin](https://github.com/enthusiastmartin))

#### Authors: 6

- [@lumir-mrkva](https://github.com/lumir-mrkva)
- Jakub Pánik ([@jak-pan](https://github.com/jak-pan))
- Jindrich Zeleny ([@green-jay](https://github.com/green-jay))
- martin fridrich ([@martinfridrich](https://github.com/martinfridrich))
- Martin Hloska ([@enthusiastmartin](https://github.com/enthusiastmartin))
- Richard Roznovjak ([@Roznovjak](https://github.com/Roznovjak))

---

# v3.0.0 (Sun Mar 21 2021)

#### 💥 Breaking Change

- refactor!: substrate 3 upgrade [#141](https://github.com/galacticcouncil/HydraDX-node/pull/141) ([@enthusiastmartin](https://github.com/enthusiastmartin) [@martinfridrich](https://github.com/martinfridrich) [@jak-pan](https://github.com/jak-pan))

#### 🚀 Enhancement

- feat: add pallet scheduler [#162](https://github.com/galacticcouncil/HydraDX-node/pull/162) ([@enthusiastmartin](https://github.com/enthusiastmartin))

#### 🐛 Bug Fix

- fix: update perf check script [#165](https://github.com/galacticcouncil/HydraDX-node/pull/165) ([@enthusiastmartin](https://github.com/enthusiastmartin))
- fix: update weights for all pallets [#164](https://github.com/galacticcouncil/HydraDX-node/pull/164) ([@enthusiastmartin](https://github.com/enthusiastmartin))
- fix(claims): update Alice's signature in claims bench test [#142](https://github.com/galacticcouncil/HydraDX-node/pull/142) ([@green-jay](https://github.com/green-jay) [@enthusiastmartin](https://github.com/enthusiastmartin))
- fix(claims): benchmarking build fix [#136](https://github.com/galacticcouncil/HydraDX-node/pull/136) ([@enthusiastmartin](https://github.com/enthusiastmartin))
- fix(amm): calculation fixes + price type change [#7](https://github.com/galacticcouncil/HydraDX-node/pull/7) (martin.hloska@topmonks.com [@jak-pan](https://github.com/jak-pan) [@enthusiastmartin](https://github.com/enthusiastmartin))

#### Refactoring

- refactor: update decimals of DOLLARS [#161](https://github.com/galacticcouncil/HydraDX-node/pull/161) ([@Roznovjak](https://github.com/Roznovjak))
- refactor: migrate pallets to new pallet macro [#153](https://github.com/galacticcouncil/HydraDX-node/pull/153) ([@enthusiastmartin](https://github.com/enthusiastmartin))
- refactor: Upgrade math crate [#131](https://github.com/galacticcouncil/HydraDX-node/pull/131) ([@RoboRambo](https://github.com/RoboRambo) [@enthusiastmartin](https://github.com/enthusiastmartin) [@jak-pan](https://github.com/jak-pan))
- style: weights template format [#140](https://github.com/galacticcouncil/HydraDX-node/pull/140) ([@RoboRambo](https://github.com/RoboRambo) [@enthusiastmartin](https://github.com/enthusiastmartin))

#### Other improvements

- ci(perf-check): benchmark tool update [#128](https://github.com/galacticcouncil/HydraDX-node/pull/128) ([@enthusiastmartin](https://github.com/enthusiastmartin) [@jak-pan](https://github.com/jak-pan))
- ci(perf-check): improved python support [#129](https://github.com/galacticcouncil/HydraDX-node/pull/129) ([@lumir-mrkva](https://github.com/lumir-mrkva) [@jak-pan](https://github.com/jak-pan) [@enthusiastmartin](https://github.com/enthusiastmartin))
- build: pin rust toolchain [#159](https://github.com/galacticcouncil/HydraDX-node/pull/159) ([@enthusiastmartin](https://github.com/enthusiastmartin) [@jak-pan](https://github.com/jak-pan))

#### 📝 Documentation

- docs: add contributing guidelines [#144](https://github.com/galacticcouncil/HydraDX-node/pull/144) ([@RoboRambo](https://github.com/RoboRambo) [@jak-pan](https://github.com/jak-pan))

#### Authors: 7

- [@lumir-mrkva](https://github.com/lumir-mrkva)
- [@RoboRambo](https://github.com/RoboRambo)
- Jakub Pánik ([@jak-pan](https://github.com/jak-pan))
- Jindrich Zeleny ([@green-jay](https://github.com/green-jay))
- Martin ([@enthusiastmartin](https://github.com/enthusiastmartin))
- martin fridrich ([@martinfridrich](https://github.com/martinfridrich))
- Richard Roznovjak ([@Roznovjak](https://github.com/Roznovjak))

---

# v2.0.0 (Sun Mar 7 2021)
