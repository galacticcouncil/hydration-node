use anchor_lang::prelude::*;
use chain_signatures::cpi::accounts::Sign as SignetSign;
use chain_signatures::program::ChainSignaturesProject;
use chain_signatures::ProgramState as SignetProgramState;

declare_id!("76SSSaQQjQ35d8shjHUsUNFwfpnJamVAiCN5hWzuF84f");

#[program]
pub mod proxy_test_cpi {
    use super::*;

    pub fn call_sign(
        ctx: Context<CallSign>,
        payload: [u8; 32],
        key_version: u32,
        path: String,
        algo: String,
        dest: String,
        params: String,
    ) -> Result<()> {
        let cpi_accounts = SignetSign {
            program_state: ctx.accounts.signet_program_state.to_account_info(),
            requester: ctx.accounts.requester.to_account_info(),
            fee_payer: match &ctx.accounts.fee_payer {
                Some(payer) => Some(payer.to_account_info()),
                None => None,
            },
            system_program: ctx.accounts.system_program.to_account_info(),
            event_authority: ctx.accounts.event_authority.to_account_info(),
            program: ctx.accounts.signet_program.to_account_info(),
        };

        let cpi_program = ctx.accounts.signet_program.to_account_info();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);

        chain_signatures::cpi::sign(cpi_ctx, payload, key_version, path, algo, dest, params)?;

        msg!("Successfully called signet program via CPI");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CallSign<'info> {
    /// The signet program we're calling via CPI
    pub signet_program: Program<'info, ChainSignaturesProject>,

    /// The signet program's state account
    #[account(
        mut,
        seeds = [b"program-state"],
        bump,
        seeds::program = signet_program.key()
    )]
    pub signet_program_state: Account<'info, SignetProgramState>,

    /// The requester making the signature request
    #[account(mut)]
    pub requester: Signer<'info>,

    /// Optional fee payer (if different from requester)
    #[account(mut)]
    pub fee_payer: Option<Signer<'info>>,

    /// System program for transfers
    pub system_program: Program<'info, System>,

    /// Event authority for CPI events
    /// CHECK: This is used by the Anchor event CPI system
    pub event_authority: AccountInfo<'info>,
}
