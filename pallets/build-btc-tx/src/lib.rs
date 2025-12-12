#![cfg_attr(not(feature = "std"), no_std)]

use frame_support::pallet_prelude::*;
use frame_system::{ensure_signed, pallet_prelude::*};
use sp_std::vec::Vec;

pub use pallet::*;

#[cfg(test)]
mod tests;

#[frame_support::pallet]
pub mod pallet {
	use super::*;
	use frame_system::pallet_prelude::BlockNumberFor; // ← Add this

	#[pallet::config]
	pub trait Config: frame_system::Config {
		#[pallet::constant]
		type MaxInputs: Get<u32>;
		#[pallet::constant]
		type MaxOutputs: Get<u32>;
	}

	#[pallet::pallet]
	pub struct Pallet<T>(_);

	/// Bitcoin script max size (standard Bitcoin limit)
	pub type MaxScriptLength = ConstU32<520>;

	/// Bitcoin UTXO input with metadata required for PSBT construction
	#[derive(Encode, Decode, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug)]
	pub struct UtxoInput {
		pub txid: [u8; 32],
		pub vout: u32,
		pub value: u64,
		pub script_pubkey: BoundedVec<u8, MaxScriptLength>,
		pub sequence: u32,
	}

	/// Bitcoin transaction output
	#[derive(Encode, Decode, TypeInfo, Clone, PartialEq, Eq, RuntimeDebug)]
	pub struct BitcoinOutput {
		pub value: u64,
		pub script_pubkey: BoundedVec<u8, MaxScriptLength>,
	}

	#[pallet::error]
	pub enum Error<T> {
		NoInputs,
		NoOutputs,
		InvalidLockTime,
		PsbtCreationFailed,
		PsbtSerializationFailed,
	}

	#[pallet::hooks]
	impl<T: Config> Hooks<BlockNumberFor<T>> for Pallet<T> {}

	impl<T: Config> Pallet<T> {
		/// Build a Bitcoin PSBT and return the serialized bytes
		///
		/// # Parameters
		/// - `origin`: The signed origin
		/// - `inputs`: UTXOs to spend
		/// - `outputs`: Transaction outputs
		/// - `lock_time`: Transaction locktime
		///
		/// # Returns
		/// Serialized PSBT bytes
		pub fn build_bitcoin_tx(
			origin: OriginFor<T>,
			inputs: BoundedVec<UtxoInput, T::MaxInputs>,
			outputs: BoundedVec<BitcoinOutput, T::MaxOutputs>,
			lock_time: u32,
		) -> Result<Vec<u8>, DispatchError> {
			ensure_signed(origin)?;

			ensure!(!inputs.is_empty(), Error::<T>::NoInputs);
			ensure!(!outputs.is_empty(), Error::<T>::NoOutputs);

			let (psbt_bytes, _) = Self::build_psbt(&inputs, &outputs, lock_time)?;
			Ok(psbt_bytes)
		}

		/// Get the transaction ID (txid) for a Bitcoin transaction
		///
		/// # Parameters
		/// - `origin`: The signed origin
		/// - `inputs`: UTXOs to spend
		/// - `outputs`: Transaction outputs
		/// - `lock_time`: Transaction locktime
		///
		/// # Returns
		/// 32-byte transaction ID (canonical, excludes witness)
		pub fn get_txid(
			origin: OriginFor<T>,
			inputs: BoundedVec<UtxoInput, T::MaxInputs>,
			outputs: BoundedVec<BitcoinOutput, T::MaxOutputs>,
			lock_time: u32,
		) -> Result<[u8; 32], DispatchError> {
			ensure_signed(origin)?;

			ensure!(!inputs.is_empty(), Error::<T>::NoInputs);
			ensure!(!outputs.is_empty(), Error::<T>::NoOutputs);

			let (_, txid) = Self::build_psbt(&inputs, &outputs, lock_time)?;
			Ok(txid)
		}

		fn build_psbt(
			inputs: &[UtxoInput],
			outputs: &[BitcoinOutput],
			lock_time: u32,
		) -> Result<(Vec<u8>, [u8; 32]), DispatchError> {
			use signet_rs::bitcoin::types::{
				Amount, Hash, LockTime, OutPoint, ScriptBuf, Sequence, TxIn, TxOut, Txid, Version, Witness,
			};
			use signet_rs::{TransactionBuilder, TxBuilder, BITCOIN};

			let tx_inputs: Vec<TxIn> = inputs
				.iter()
				.map(|input| TxIn {
					previous_output: OutPoint::new(Txid(Hash(input.txid)), input.vout),
					script_sig: ScriptBuf::default(),
					sequence: Sequence(input.sequence),
					witness: Witness::default(),
				})
				.collect();

			let tx_outputs: Vec<TxOut> = outputs
				.iter()
				.map(|output| TxOut {
					value: Amount::from_sat(output.value),
					script_pubkey: ScriptBuf(output.script_pubkey.to_vec()),
				})
				.collect();

			let lock_time_parsed = if lock_time < 500_000_000 {
				LockTime::from_height(lock_time).map_err(|_| Error::<T>::InvalidLockTime)?
			} else {
				LockTime::from_time(lock_time).map_err(|_| Error::<T>::InvalidLockTime)?
			};

			let tx = TransactionBuilder::new::<BITCOIN>()
				.version(Version::Two)
				.inputs(tx_inputs)
				.outputs(tx_outputs)
				.lock_time(lock_time_parsed)
				.build();

			// Extract txid before creating PSBT
			let txid = tx.compute_txid();
			let mut txid_bytes = txid.as_byte_array();
			txid_bytes.reverse();

			let mut psbt = tx.to_psbt();

			for (i, input) in inputs.iter().enumerate() {
				psbt.update_input_with_witness_utxo(i, input.script_pubkey.to_vec(), input.value)
					.map_err(|_| Error::<T>::PsbtCreationFailed)?;

				psbt.update_input_with_sighash_type(i, 1)
					.map_err(|_| Error::<T>::PsbtCreationFailed)?;
			}

			let psbt_bytes = psbt.serialize().map_err(|_| Error::<T>::PsbtSerializationFailed)?;

			Ok((psbt_bytes, txid_bytes))
		}
	}
}
