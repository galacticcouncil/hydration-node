import { Config } from './types'

import hydraDXConfig from './hydraDX'
import polkadotConfig from './polkadot'

const all = {
  hydraDX: hydraDXConfig,
  polkadot: polkadotConfig,
} satisfies Record<string, Config>

export default all
