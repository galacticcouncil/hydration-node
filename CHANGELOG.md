# v9.0.0 (Tue May 25 2021)

#### 🚀 Enhancement

- feat!: add utility pallet to allow batch txs execution [#246](https://github.com/galacticcouncil/HydraDX-node/pull/246) ([@enthusiastmartin](https://github.com/enthusiastmartin) [@lumir-mrkva](https://github.com/lumir-mrkva))

#### Refactoring

- refactor!: revert price type to fixedu128 [#259](https://github.com/galacticcouncil/HydraDX-node/pull/259) ([@enthusiastmartin](https://github.com/enthusiastmartin))
- refactor: pallet improvements and xyk pallet [#242](https://github.com/galacticcouncil/HydraDX-node/pull/242) ([@enthusiastmartin](https://github.com/enthusiastmartin))
- refactor(build-script): allow to specify runtime [#254](https://github.com/galacticcouncil/HydraDX-node/pull/254) ([@enthusiastmartin](https://github.com/enthusiastmartin))

#### Authors: 2

- [@lumir-mrkva](https://github.com/lumir-mrkva)
- Martin Hloska ([@enthusiastmartin](https://github.com/enthusiastmartin))

---

# v8.0.0 (Tue May 11 2021)

#### 💥 Breaking Change

- feat!: genesis3 reborn [#253](https://github.com/galacticcouncil/HydraDX-node/pull/253) ([@jak-pan](https://github.com/jak-pan))

#### Authors: 1

- Jakub Pánik ([@jak-pan](https://github.com/jak-pan))

---

# v7.0.0 (Sun May 09 2021)

#### 💥 Breaking Change

- feat!: genesis 3 [#251](https://github.com/galacticcouncil/HydraDX-node/pull/251) ([@jak-pan](https://github.com/jak-pan))

#### 🐛 Bug Fix

- fix(chore): discard patch for libsrock-db [#247](https://github.com/galacticcouncil/HydraDX-node/pull/247) ([@jak-pan](https://github.com/jak-pan))

#### Authors: 1

- Jakub Pánik ([@jak-pan](https://github.com/jak-pan))

---

# v6.0.0 (Wed May 05 2021)

#### 💥 Breaking Change

- fix!: add election fallback on-chain [#221](https://github.com/galacticcouncil/HydraDX-node/pull/221) ([@jak-pan](https://github.com/jak-pan))
- fix!: set babe epoch config at genesis [#241](https://github.com/galacticcouncil/HydraDX-node/pull/241) ([@enthusiastmartin](https://github.com/enthusiastmartin) [@jak-pan](https://github.com/jak-pan))

#### 🚀 Enhancement

- feat: change Price type [#235](https://github.com/galacticcouncil/HydraDX-node/pull/235) ([@martinfridrich](https://github.com/martinfridrich) [@lumir-mrkva](https://github.com/lumir-mrkva))
- feat: add tests for time units [#212](https://github.com/galacticcouncil/HydraDX-node/pull/212) ([@green-jay](https://github.com/green-jay) [@lumir-mrkva](https://github.com/lumir-mrkva))

#### 🐛 Bug Fix

- fix(multi-payment): move balances dependency to dev-dependency [#239](https://github.com/galacticcouncil/HydraDX-node/pull/239) ([@enthusiastmartin](https://github.com/enthusiastmartin))
- fix(node): configure justification import for full node [#225](https://github.com/galacticcouncil/HydraDX-node/pull/225) ([@andresilva](https://github.com/andresilva) [@lumir-mrkva](https://github.com/lumir-mrkva))
- fix(ci): tag version workflow [#216](https://github.com/galacticcouncil/HydraDX-node/pull/216) ([@lumir-mrkva](https://github.com/lumir-mrkva))

#### Refactoring

- refactor(amm): changed event names to past tense [#215](https://github.com/galacticcouncil/HydraDX-node/pull/215) ([@jareknowotka](https://github.com/jareknowotka) [@lumir-mrkva](https://github.com/lumir-mrkva))

#### Other improvements

- chore: Substrate update [#224](https://github.com/galacticcouncil/HydraDX-node/pull/224) ([@enthusiastmartin](https://github.com/enthusiastmartin) [@lumir-mrkva](https://github.com/lumir-mrkva))
- ci: Build workflow from fork PR [#230](https://github.com/galacticcouncil/HydraDX-node/pull/230) ([@lumir-mrkva](https://github.com/lumir-mrkva))
- ci: automatically creates an EC2 instance for builds [#220](https://github.com/galacticcouncil/HydraDX-node/pull/220) ([@lumir-mrkva](https://github.com/lumir-mrkva) ayoub.fakir@vodafoneziggo.com [@fakirAyoub](https://github.com/fakirAyoub))

#### 📝 Documentation

- docs: Code docs update [#238](https://github.com/galacticcouncil/HydraDX-node/pull/238) ([@enthusiastmartin](https://github.com/enthusiastmartin) [@jak-pan](https://github.com/jak-pan))

#### Authors: 9

- [@jareknowotka](https://github.com/jareknowotka)
- [@lumir-mrkva](https://github.com/lumir-mrkva)
- André Silva ([@andresilva](https://github.com/andresilva))
- Ayoub (ayoub.fakir@vodafoneziggo.com)
- Ayoub Fakir ([@fakirAyoub](https://github.com/fakirAyoub))
- Jakub Pánik ([@jak-pan](https://github.com/jak-pan))
- Jindrich Zeleny ([@green-jay](https://github.com/green-jay))
- martin fridrich ([@martinfridrich](https://github.com/martinfridrich))
- Martin Hloska ([@enthusiastmartin](https://github.com/enthusiastmartin))

---

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
