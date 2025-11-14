//! Sig.Network signing program for accepting signature requests and providing responses from the Sig.Network.

#![allow(unexpected_cfgs)]
use anchor_lang::prelude::*;

declare_id!("4uvZW8K4g4jBg7dzPNbb9XDxJLFBK7V6iC76uofmYvEU");

/**
 * @title Sig.Network signing program
 * @dev Program for accepting signature requests and providing responses from the Sig.Network.
 */
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, Debug, PartialEq)]
#[repr(u8)]
pub enum SerializationFormat {
    Borsh = 0,
    AbiJson = 1,
}

#[program]
pub mod chain_signatures_project {
    use super::*;
    /**
     * @dev Function to initialize the program state.
     * @param signature_deposit The deposit required for signature requests.
     * @param chain_id The CAIP-2 chain identifier.
     */
    pub fn initialize(
        ctx: Context<Initialize>,
        signature_deposit: u64,
        chain_id: String,
    ) -> Result<()> {
        let program_state = &mut ctx.accounts.program_state;
        program_state.admin = ctx.accounts.admin.key();
        program_state.signature_deposit = signature_deposit;
        program_state.chain_id = chain_id;

        Ok(())
    }

    /**
     * @dev Function to set the signature deposit amount.
     * @param new_deposit The new deposit amount.
     */
    pub fn update_deposit(ctx: Context<AdminOnly>, new_deposit: u64) -> Result<()> {
        let program_state = &mut ctx.accounts.program_state;
        let old_deposit = program_state.signature_deposit;
        program_state.signature_deposit = new_deposit;

        emit!(DepositUpdatedEvent {
            old_deposit,
            new_deposit,
        });

        Ok(())
    }

    /**
     * @dev Function to withdraw funds from the program.
     * @param amount The amount to withdraw.
     */
    pub fn withdraw_funds(ctx: Context<WithdrawFunds>, amount: u64) -> Result<()> {
        let program_state = &ctx.accounts.program_state;
        let recipient = &ctx.accounts.recipient;

        let program_state_info = program_state.to_account_info();
        require!(
            program_state_info.lamports() >= amount,
            ChainSignaturesError::InsufficientFunds
        );

        require!(
            recipient.key() != Pubkey::default(),
            ChainSignaturesError::InvalidRecipient
        );

        // Transfer funds from program_state to recipient
        **program_state_info.try_borrow_mut_lamports()? -= amount;
        **recipient.try_borrow_mut_lamports()? += amount;

        emit!(FundsWithdrawnEvent {
            amount,
            recipient: recipient.key(),
        });

        Ok(())
    }

    /**
     * @dev Function to request a signature.
     * @param payload The payload to be signed.
     * @param key_version The version of the key used for signing.
     * @param path The derivation path for the user account.
     * @param algo The algorithm used for signing.
     * @param dest The response destination.
     * @param params Additional parameters.
     */
    pub fn sign(
        ctx: Context<Sign>,
        payload: [u8; 32],
        key_version: u32,
        path: String,
        algo: String,
        dest: String,
        params: String,
    ) -> Result<()> {
        let program_state = &ctx.accounts.program_state;
        let requester = &ctx.accounts.requester;
        let system_program = &ctx.accounts.system_program;

        let payer = match &ctx.accounts.fee_payer {
            Some(fee_payer) => fee_payer.to_account_info(),
            None => requester.to_account_info(),
        };

        require!(
            payer.lamports() >= program_state.signature_deposit,
            ChainSignaturesError::InsufficientDeposit
        );

        let transfer_instruction = anchor_lang::system_program::Transfer {
            from: payer,
            to: program_state.to_account_info(),
        };

        anchor_lang::system_program::transfer(
            CpiContext::new(system_program.to_account_info(), transfer_instruction),
            program_state.signature_deposit,
        )?;

        emit_cpi!(SignatureRequestedEvent {
            sender: *requester.key,
            payload,
            key_version,
            deposit: program_state.signature_deposit,
            chain_id: program_state.chain_id.clone(),
            path,
            algo,
            dest,
            params,
            fee_payer: match &ctx.accounts.fee_payer {
                Some(payer) => Some(*payer.key),
                None => None,
            },
        });

        Ok(())
    }

    pub fn sign_respond(
        ctx: Context<SignRespond>,
        serialized_transaction: Vec<u8>,
        slip44_chain_id: u32,
        key_version: u32,
        path: String,
        algo: String,
        dest: String,
        params: String,
        explorer_deserialization_format: SerializationFormat,
        explorer_deserialization_schema: Vec<u8>,
        callback_serialization_format: SerializationFormat,
        callback_serialization_schema: Vec<u8>,
    ) -> Result<()> {
        let program_state = &ctx.accounts.program_state;
        let requester = &ctx.accounts.requester;
        let system_program = &ctx.accounts.system_program;

        let payer = match &ctx.accounts.fee_payer {
            Some(fee_payer) => fee_payer.to_account_info(),
            None => requester.to_account_info(),
        };

        require!(
            payer.lamports() >= program_state.signature_deposit,
            ChainSignaturesError::InsufficientDeposit
        );

        require!(
            !serialized_transaction.is_empty(),
            ChainSignaturesError::InvalidTransaction
        );

        let transfer_instruction = anchor_lang::system_program::Transfer {
            from: payer,
            to: program_state.to_account_info(),
        };

        anchor_lang::system_program::transfer(
            CpiContext::new(system_program.to_account_info(), transfer_instruction),
            program_state.signature_deposit,
        )?;

        emit_cpi!(SignRespondRequestedEvent {
            sender: *requester.key,
            transaction_data: serialized_transaction,
            slip44_chain_id,
            key_version,
            deposit: program_state.signature_deposit,
            path,
            algo,
            dest,
            params,
            explorer_deserialization_format: explorer_deserialization_format as u8,
            explorer_deserialization_schema,
            callback_serialization_format: callback_serialization_format as u8,
            callback_serialization_schema
        });

        Ok(())
    }

    /**
     * @dev Function to respond to signature requests.
     * @param request_ids The array of request IDs.
     * @param signatures The array of signature responses.
     */
    pub fn respond(
        ctx: Context<Respond>,
        request_ids: Vec<[u8; 32]>,
        signatures: Vec<Signature>,
    ) -> Result<()> {
        require!(
            request_ids.len() == signatures.len(),
            ChainSignaturesError::InvalidInputLength
        );

        for i in 0..request_ids.len() {
            emit!(SignatureRespondedEvent {
                request_id: request_ids[i],
                responder: *ctx.accounts.responder.key,
                signature: signatures[i].clone(),
            });
        }

        Ok(())
    }

    /**
     * @dev Function to emit signature generation errors.
     * @param errors The array of signature generation errors.
     */
    pub fn respond_error(ctx: Context<RespondError>, errors: Vec<ErrorResponse>) -> Result<()> {
        for error in errors {
            emit!(SignatureErrorEvent {
                request_id: error.request_id,
                responder: *ctx.accounts.responder.key,
                error: error.error_message,
            });
        }

        Ok(())
    }

    /**
     * @dev Function to get the current signature deposit amount.
     * @return The current signature deposit amount.
     */
    pub fn get_signature_deposit(ctx: Context<GetSignatureDeposit>) -> Result<u64> {
        let program_state = &ctx.accounts.program_state;
        Ok(program_state.signature_deposit)
    }

    pub fn read_respond(
        ctx: Context<ReadRespond>,
        request_id: [u8; 32],
        serialized_output: Vec<u8>,
        signature: Signature,
    ) -> Result<()> {
        // The signature should be an ECDSA signature over keccak256(request_id || serialized_output)

        // only possible error responses // (this tx could never happen):
        // - nonce too low
        // - balance too low
        // - literal on chain error

        emit!(ReadRespondedEvent {
            request_id,
            responder: *ctx.accounts.responder.key,
            serialized_output,
            signature,
        });

        Ok(())
    }
}

#[account]
pub struct ProgramState {
    pub admin: Pubkey,
    pub signature_deposit: u64,
    pub chain_id: String,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct AffinePoint {
    pub x: [u8; 32],
    pub y: [u8; 32],
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Signature {
    pub big_r: AffinePoint,
    pub s: [u8; 32],
    pub recovery_id: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct ErrorResponse {
    pub request_id: [u8; 32],
    pub error_message: String,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = admin,
        space = 8 + 32 + 8 + 4 + 128, // discriminator + admin + deposit + string length + max chain_id length
        seeds = [b"program-state"],
        bump
    )]
    pub program_state: Account<'info, ProgramState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct AdminOnly<'info> {
    #[account(
        mut,
        seeds = [b"program-state"],
        bump,
        has_one = admin @ ChainSignaturesError::Unauthorized
    )]
    pub program_state: Account<'info, ProgramState>,
    #[account(mut)]
    pub admin: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct WithdrawFunds<'info> {
    #[account(
        mut,
        seeds = [b"program-state"],
        bump,
        has_one = admin @ ChainSignaturesError::Unauthorized
    )]
    pub program_state: Account<'info, ProgramState>,

    #[account(mut)]
    pub admin: Signer<'info>,

    /// CHECK: The safety check is performed in the withdraw_funds
    /// function by checking it is not the zero address.
    #[account(mut)]
    pub recipient: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
}

#[event_cpi]
#[derive(Accounts)]
pub struct Sign<'info> {
    #[account(mut, seeds = [b"program-state"], bump)]
    pub program_state: Account<'info, ProgramState>,
    #[account(mut)]
    pub requester: Signer<'info>,
    #[account(mut)]
    pub fee_payer: Option<Signer<'info>>,
    pub system_program: Program<'info, System>,
}

#[event_cpi]
#[derive(Accounts)]
pub struct SignRespond<'info> {
    #[account(mut, seeds = [b"program-state"], bump)]
    pub program_state: Account<'info, ProgramState>,
    #[account(mut)]
    pub requester: Signer<'info>,
    #[account(mut)]
    pub fee_payer: Option<Signer<'info>>,
    pub system_program: Program<'info, System>,
    pub instructions: Option<AccountInfo<'info>>,
}

#[derive(Accounts)]
pub struct Respond<'info> {
    pub responder: Signer<'info>,
}

#[derive(Accounts)]
pub struct RespondError<'info> {
    pub responder: Signer<'info>,
}

#[derive(Accounts)]
pub struct GetSignatureDeposit<'info> {
    #[account(seeds = [b"program-state"], bump)]
    pub program_state: Account<'info, ProgramState>,
}

#[derive(Accounts)]
pub struct ReadRespond<'info> {
    pub responder: Signer<'info>,
}

/**
 * @dev Emitted when a signature is requested.
 * @param sender The address of the sender.
 * @param payload The payload to be signed.
 * @param key_version The version of the key used for signing.
 * @param deposit The deposit amount.
 * @param chain_id The CAIP-2 ID of the blockchain.
 * @param path The derivation path for the user account.
 * @param algo The algorithm used for signing.
 * @param dest The response destination.
 * @param params Additional parameters.
 * @param fee_payer Optional fee payer account.
 */
#[event]
pub struct SignatureRequestedEvent {
    pub sender: Pubkey,
    pub payload: [u8; 32],
    pub key_version: u32,
    pub deposit: u64,
    pub chain_id: String,
    pub path: String,
    pub algo: String,
    pub dest: String,
    pub params: String,
    pub fee_payer: Option<Pubkey>,
}

/**
 * @dev Emitted when a signature response is received.
 * @notice Any address can emit this event. Clients should always verify the validity of the signature.
 * @param request_id The ID of the request. Must be calculated off-chain.
 * @param responder The address of the responder.
 * @param signature The signature response.
 */
#[event]
pub struct SignRespondRequestedEvent {
    pub sender: Pubkey,
    pub transaction_data: Vec<u8>,
    pub slip44_chain_id: u32,
    pub key_version: u32,
    pub deposit: u64,
    pub path: String,
    pub algo: String,
    pub dest: String,
    pub params: String,
    pub explorer_deserialization_format: u8,
    pub explorer_deserialization_schema: Vec<u8>,
    pub callback_serialization_format: u8,
    pub callback_serialization_schema: Vec<u8>,
}

#[event]
pub struct SignatureRespondedEvent {
    pub request_id: [u8; 32],
    pub responder: Pubkey,
    pub signature: Signature,
}

/**
 * @dev Emitted when a signature error is received.
 * @notice Any address can emit this event. Do not rely on it for business logic.
 * @param request_id The ID of the request. Must be calculated off-chain.
 * @param responder The address of the responder.
 * @param error The error message.
 */
#[event]
pub struct SignatureErrorEvent {
    pub request_id: [u8; 32],
    pub responder: Pubkey,
    pub error: String,
}

/**
 * @dev Emitted when a read response is received.
 * @param request_id The ID of the request. Must be calculated off-chain.
 * @param responder The address of the responder.
 * @param serialized_output The serialized output.
 * @param signature The signature.
 */
#[event]
pub struct ReadRespondedEvent {
    pub request_id: [u8; 32],
    pub responder: Pubkey,
    pub serialized_output: Vec<u8>,
    pub signature: Signature,
}

/**
 * @dev Emitted when the deposit amount is updated.
 * @param old_deposit The previous deposit amount.
 * @param new_deposit The new deposit amount.
 */
#[event]
pub struct DepositUpdatedEvent {
    pub old_deposit: u64,
    pub new_deposit: u64,
}

/**
 * @dev Emitted when a withdrawal is made.
 * @param amount The amount withdrawn.
 * @param recipient The address of the recipient.
 */
#[event]
pub struct FundsWithdrawnEvent {
    pub amount: u64,
    pub recipient: Pubkey,
}

#[error_code]
pub enum ChainSignaturesError {
    #[msg("Insufficient deposit amount")]
    InsufficientDeposit,
    #[msg("Arrays must have the same length")]
    InvalidInputLength,
    #[msg("Unauthorized access")]
    Unauthorized,
    #[msg("Insufficient funds for withdrawal")]
    InsufficientFunds,
    #[msg("Invalid recipient address")]
    InvalidRecipient,
    #[msg("Invalid transaction data")]
    InvalidTransaction,
    #[msg("Missing instruction sysvar")]
    MissingInstructionSysvar,
}
