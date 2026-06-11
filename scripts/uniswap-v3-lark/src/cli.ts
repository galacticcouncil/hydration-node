import { Command } from 'commander'
import { deploy } from './commands/deploy'
import { fund } from './commands/fund'
import { preflight } from './commands/preflight'
import { seed } from './commands/seed'
import { setAddresses } from './commands/setAddresses'
import { swapTest } from './commands/swapTest'
import { whitelist } from './commands/whitelist'

function run(action: () => Promise<void>): () => Promise<void> {
  return async () => {
    try {
      await action()
      process.exit(0)
    } catch (error) {
      console.error('FAIL:', error instanceof Error ? error.message : error)
      process.exit(1)
    }
  }
}

const program = new Command()
program
  .name('uniswap-v3-lark')
  .description('Deploy and validate Uniswap v3 as a native router venue on Hydration (lark)')

program
  .command('fund')
  .description('mint gas + pool assets to the deployer (Root referendum)')
  .action(run(fund))
program
  .command('whitelist')
  .description('whitelist the deployer for EVM contract creation (Root referendum)')
  .action(run(whitelist))
program
  .command('deploy')
  .description('deploy the Uniswap v3 stack via @uniswap/deploy-v3')
  .action(run(deploy))
program
  .command('seed')
  .description('create + initialize + fund the pool over precompile tokens')
  .action(run(seed))
program
  .command('set-addresses')
  .description('set the runtime parameters to the deployed contracts (Root referendum)')
  .action(run(setAddresses))
program
  .command('preflight')
  .description('check readiness for the swap test')
  .action(run(preflight))
program
  .command('swap-test')
  .description('run the mixed Omnipool + Uniswap v3 router swap')
  .action(run(swapTest))

program.parseAsync(process.argv)
