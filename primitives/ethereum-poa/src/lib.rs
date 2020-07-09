// Copyright 2019-2020 Parity Technologies (UK) Ltd.
// This file is part of Parity Bridges Common.

// Parity Bridges Common is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Parity Bridges Common is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Parity Bridges Common.  If not, see <http://www.gnu.org/licenses/>.

#![cfg_attr(not(feature = "std"), no_std)]
// RuntimeApi generated functions
#![allow(clippy::too_many_arguments)]

pub use parity_bytes::Bytes;
pub use primitive_types::{H160, H256, H512, U128, U256};
pub use rlp::encode as rlp_encode;

use codec::{Decode, Encode};
use ethbloom::{Bloom as EthBloom, Input as BloomInput};
use fixed_hash::construct_fixed_hash;
use rlp::{Decodable, DecoderError, Rlp, RlpStream};
use sp_io::hashing::keccak_256;
use sp_runtime::RuntimeDebug;
use sp_std::prelude::*;

use impl_rlp::impl_fixed_hash_rlp;
#[cfg(feature = "std")]
use impl_serde::impl_fixed_hash_serde;
#[cfg(feature = "std")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "std")]
use serde_big_array::big_array;

construct_fixed_hash! { pub struct H520(65); }
impl_fixed_hash_rlp!(H520, 65);
#[cfg(feature = "std")]
impl_fixed_hash_serde!(H520, 65);

/// Raw (RLP-encoded) ethereum transaction.
pub type RawTransaction = Vec<u8>;

/// An ethereum address.
pub type Address = H160;

#[cfg(any(feature = "test-helpers", test))]
pub mod signatures;

/// Complete header id.
#[derive(Encode, Decode, Default, RuntimeDebug, PartialEq, Clone, Copy)]
pub struct HeaderId {
	/// Header number.
	pub number: u64,
	/// Header hash.
	pub hash: H256,
}

/// An Aura header.
#[derive(Clone, Default, Encode, Decode, PartialEq, RuntimeDebug)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Header {
	/// Parent block hash.
	pub parent_hash: H256,
	/// Block timestamp.
	pub timestamp: u64,
	/// Block number.
	pub number: u64,
	/// Block author.
	pub author: Address,

	/// Transactions root.
	pub transactions_root: H256,
	/// Block uncles hash.
	pub uncles_hash: H256,
	/// Block extra data.
	pub extra_data: Bytes,

	/// State root.
	pub state_root: H256,
	/// Block receipts root.
	pub receipts_root: H256,
	/// Block bloom.
	pub log_bloom: Bloom,
	/// Gas used for contracts execution.
	pub gas_used: U256,
	/// Block gas limit.
	pub gas_limit: U256,

	/// Block difficulty.
	pub difficulty: U256,
	/// Vector of post-RLP-encoded fields.
	pub seal: Vec<Bytes>,
}

/// Parsed ethereum transaction.
#[derive(PartialEq, RuntimeDebug)]
pub struct Transaction {
	/// Sender address.
	pub sender: Address,
	/// Unsigned portion of ethereum transaction.
	pub unsigned: UnsignedTransaction,
}

/// Unsigned portion of ethereum transaction.
#[derive(PartialEq, RuntimeDebug)]
#[cfg_attr(test, derive(Clone))]
pub struct UnsignedTransaction {
	/// Sender nonce.
	pub nonce: U256,
	/// Gas price.
	pub gas_price: U256,
	/// Gas limit.
	pub gas: U256,
	/// Transaction destination address. None if it is contract creation transaction.
	pub to: Option<Address>,
	/// Transaction value.
	pub value: U256,
	/// Transaction payload.
	pub payload: Bytes,
}

/// Information describing execution of a transaction.
#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug)]
pub struct Receipt {
	/// The total gas used in the block following execution of the transaction.
	pub gas_used: U256,
	/// The OR-wide combination of all logs' blooms for this transaction.
	pub log_bloom: Bloom,
	/// The logs stemming from this transaction.
	pub logs: Vec<LogEntry>,
	/// Transaction outcome.
	pub outcome: TransactionOutcome,
}

/// Transaction outcome store in the receipt.
#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug)]
pub enum TransactionOutcome {
	/// Status and state root are unknown under EIP-98 rules.
	Unknown,
	/// State root is known. Pre EIP-98 and EIP-658 rules.
	StateRoot(H256),
	/// Status code is known. EIP-658 rules.
	StatusCode(u8),
}

/// A record of execution for a `LOG` operation.
#[derive(Clone, Encode, Decode, PartialEq, RuntimeDebug)]
pub struct LogEntry {
	/// The address of the contract executing at the point of the `LOG` operation.
	pub address: Address,
	/// The topics associated with the `LOG` operation.
	pub topics: Vec<H256>,
	/// The data associated with the `LOG` operation.
	pub data: Bytes,
}

/// Logs bloom.
#[derive(Clone, Encode, Decode)]
#[cfg_attr(feature = "std", derive(Serialize, Deserialize))]
pub struct Bloom(#[cfg_attr(feature = "std", serde(with = "BigArray"))] [u8; 256]);

#[cfg(feature = "std")]
big_array! { BigArray; }

/// An empty step message that is included in a seal, the only difference is that it doesn't include
/// the `parent_hash` in order to save space. The included signature is of the original empty step
/// message, which can be reconstructed by using the parent hash of the block in which this sealed
/// empty message is included.
pub struct SealedEmptyStep {
	/// Signature of the original message author.
	pub signature: H520,
	/// The step this message is generated for.
	pub step: u64,
}

impl Header {
	/// Compute id of this header.
	pub fn compute_id(&self) -> HeaderId {
		HeaderId {
			number: self.number,
			hash: self.compute_hash(),
		}
	}

	/// Compute hash of this header (keccak of the RLP with seal).
	pub fn compute_hash(&self) -> H256 {
		keccak_256(&self.rlp(true)).into()
	}

	/// Get id of this header' parent. Returns None if this is genesis header.
	pub fn parent_id(&self) -> Option<HeaderId> {
		self.number.checked_sub(1).map(|parent_number| HeaderId {
			number: parent_number,
			hash: self.parent_hash,
		})
	}

	/// Check if passed transactions receipts are matching receipts root in this header.
	pub fn verify_receipts_root(&self, receipts: &[Receipt]) -> bool {
		verify_merkle_proof(self.receipts_root, receipts.iter().map(|r| r.rlp()))
	}

	/// Check if passed transactions are matching transactions root in this header.
	pub fn verify_transactions_root(&self, transactions: &[RawTransaction]) -> bool {
		verify_merkle_proof(self.transactions_root, transactions.iter())
	}

	/// Gets the seal hash of this header.
	pub fn seal_hash(&self, include_empty_steps: bool) -> Option<H256> {
		Some(match include_empty_steps {
			true => {
				let mut message = self.compute_hash().as_bytes().to_vec();
				message.extend_from_slice(self.seal.get(2)?);
				keccak_256(&message).into()
			}
			false => keccak_256(&self.rlp(false)).into(),
		})
	}

	/// Get step this header is generated for.
	pub fn step(&self) -> Option<u64> {
		self.seal.get(0).map(|x| Rlp::new(&x)).and_then(|x| x.as_val().ok())
	}

	/// Get header author' signature.
	pub fn signature(&self) -> Option<H520> {
		self.seal.get(1).and_then(|x| Rlp::new(x).as_val().ok())
	}

	/// Extracts the empty steps from the header seal.
	pub fn empty_steps(&self) -> Option<Vec<SealedEmptyStep>> {
		self.seal
			.get(2)
			.and_then(|x| Rlp::new(x).as_list::<SealedEmptyStep>().ok())
	}

	/// Returns header RLP with or without seals.
	fn rlp(&self, with_seal: bool) -> Bytes {
		let mut s = RlpStream::new();
		if with_seal {
			s.begin_list(13 + self.seal.len());
		} else {
			s.begin_list(13);
		}

		s.append(&self.parent_hash);
		s.append(&self.uncles_hash);
		s.append(&self.author);
		s.append(&self.state_root);
		s.append(&self.transactions_root);
		s.append(&self.receipts_root);
		s.append(&EthBloom::from(self.log_bloom.0));
		s.append(&self.difficulty);
		s.append(&self.number);
		s.append(&self.gas_limit);
		s.append(&self.gas_used);
		s.append(&self.timestamp);
		s.append(&self.extra_data);

		if with_seal {
			for b in &self.seal {
				s.append_raw(b, 1);
			}
		}

		s.out()
	}
}

impl UnsignedTransaction {
	/// Decode unsigned portion of raw transaction RLP.
	pub fn decode(raw_tx: &[u8]) -> Result<Self, DecoderError> {
		let tx_rlp = Rlp::new(raw_tx);
		let to = tx_rlp.at(3)?;
		Ok(UnsignedTransaction {
			nonce: tx_rlp.val_at(0)?,
			gas_price: tx_rlp.val_at(1)?,
			gas: tx_rlp.val_at(2)?,
			to: match to.is_empty() {
				false => Some(to.as_val()?),
				true => None,
			},
			value: tx_rlp.val_at(4)?,
			payload: tx_rlp.val_at(5)?,
		})
	}

	/// Returns message that has to be signed to sign this transaction.
	pub fn message(&self, chain_id: Option<u64>) -> H256 {
		keccak_256(&self.rlp(chain_id)).into()
	}

	/// Returns unsigned transaction RLP.
	pub fn rlp(&self, chain_id: Option<u64>) -> Bytes {
		let mut stream = RlpStream::new_list(if chain_id.is_some() { 9 } else { 6 });
		self.rlp_to(chain_id, &mut stream);
		stream.out()
	}

	/// Encode to given rlp stream.
	pub fn rlp_to(&self, chain_id: Option<u64>, stream: &mut RlpStream) {
		stream.append(&self.nonce);
		stream.append(&self.gas_price);
		stream.append(&self.gas);
		match self.to {
			Some(to) => stream.append(&to),
			None => stream.append(&""),
		};
		stream.append(&self.value);
		stream.append(&self.payload);
		if let Some(chain_id) = chain_id {
			stream.append(&chain_id);
			stream.append(&0u8);
			stream.append(&0u8);
		}
	}
}

impl Receipt {
	/// Returns receipt RLP.
	fn rlp(&self) -> Bytes {
		let mut s = RlpStream::new();
		match self.outcome {
			TransactionOutcome::Unknown => {
				s.begin_list(3);
			}
			TransactionOutcome::StateRoot(ref root) => {
				s.begin_list(4);
				s.append(root);
			}
			TransactionOutcome::StatusCode(ref status_code) => {
				s.begin_list(4);
				s.append(status_code);
			}
		}
		s.append(&self.gas_used);
		s.append(&EthBloom::from(self.log_bloom.0));

		s.begin_list(self.logs.len());
		for log in &self.logs {
			s.begin_list(3);
			s.append(&log.address);
			s.begin_list(log.topics.len());
			for topic in &log.topics {
				s.append(topic);
			}
			s.append(&log.data);
		}

		s.out()
	}
}

impl SealedEmptyStep {
	/// Returns message that has to be signed by the validator.
	pub fn message(&self, parent_hash: &H256) -> H256 {
		let mut message = RlpStream::new_list(2);
		message.append(&self.step);
		message.append(parent_hash);
		keccak_256(&message.out()).into()
	}

	/// Returns rlp for the vector of empty steps (we only do encoding in tests).
	#[cfg(feature = "test-helpers")]
	pub fn rlp_of(empty_steps: &[SealedEmptyStep]) -> Bytes {
		let mut s = RlpStream::new();
		s.begin_list(empty_steps.len());
		for empty_step in empty_steps {
			s.begin_list(2).append(&empty_step.signature).append(&empty_step.step);
		}
		s.out()
	}
}

impl Decodable for SealedEmptyStep {
	fn decode(rlp: &Rlp) -> Result<Self, DecoderError> {
		let signature: H520 = rlp.val_at(0)?;
		let step = rlp.val_at(1)?;

		Ok(SealedEmptyStep { signature, step })
	}
}

impl LogEntry {
	/// Calculates the bloom of this log entry.
	pub fn bloom(&self) -> Bloom {
		let eth_bloom =
			self.topics
				.iter()
				.fold(EthBloom::from(BloomInput::Raw(self.address.as_bytes())), |mut b, t| {
					b.accrue(BloomInput::Raw(t.as_bytes()));
					b
				});
		Bloom(*eth_bloom.data())
	}
}

impl Bloom {
	/// Returns true if this bloom has all bits from the other set.
	pub fn contains(&self, other: &Bloom) -> bool {
		self.0.iter().zip(other.0.iter()).all(|(l, r)| (l & r) == *r)
	}
}

impl<'a> From<&'a [u8; 256]> for Bloom {
	fn from(buffer: &'a [u8; 256]) -> Bloom {
		Bloom(*buffer)
	}
}

impl PartialEq<Bloom> for Bloom {
	fn eq(&self, other: &Bloom) -> bool {
		self.0.iter().zip(other.0.iter()).all(|(l, r)| l == r)
	}
}

impl Default for Bloom {
	fn default() -> Self {
		Bloom([0; 256])
	}
}

#[cfg(feature = "std")]
impl std::fmt::Debug for Bloom {
	fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::fmt::Result {
		fmt.debug_struct("Bloom").finish()
	}
}

/// Decode Ethereum transaction.
pub fn transaction_decode(raw_tx: &[u8]) -> Result<Transaction, rlp::DecoderError> {
	// parse transaction fields
	let unsigned = UnsignedTransaction::decode(raw_tx)?;
	let tx_rlp = Rlp::new(raw_tx);
	let v: u64 = tx_rlp.val_at(6)?;
	let r: U256 = tx_rlp.val_at(7)?;
	let s: U256 = tx_rlp.val_at(8)?;

	// reconstruct signature
	let mut signature = [0u8; 65];
	let (chain_id, v) = match v {
		v if v == 27u64 => (None, 0),
		v if v == 28u64 => (None, 1),
		v if v >= 35u64 => (Some((v - 35) / 2), ((v - 1) % 2) as u8),
		_ => (None, 4),
	};
	r.to_big_endian(&mut signature[0..32]);
	s.to_big_endian(&mut signature[32..64]);
	signature[64] = v;

	// reconstruct message that has been signed
	let message = unsigned.message(chain_id);

	// recover tx sender
	let sender_public = sp_io::crypto::secp256k1_ecdsa_recover(&signature, &message.as_fixed_bytes())
		.map_err(|_| rlp::DecoderError::Custom("Failed to recover transaction sender"))?;
	let sender_address = public_to_address(&sender_public);

	Ok(Transaction {
		sender: sender_address,
		unsigned,
	})
}

/// Convert public key into corresponding ethereum address.
pub fn public_to_address(public: &[u8; 64]) -> Address {
	let hash = keccak_256(public);
	let mut result = Address::zero();
	result.as_bytes_mut().copy_from_slice(&hash[12..]);
	result
}

/// Verify ethereum merkle proof.
fn verify_merkle_proof<T: AsRef<[u8]>>(expected_root: H256, items: impl Iterator<Item = T>) -> bool {
	compute_merkle_root(items) == expected_root
}

/// Compute ethereum merkle root.
pub fn compute_merkle_root<T: AsRef<[u8]>>(items: impl Iterator<Item = T>) -> H256 {
	struct Keccak256Hasher;

	impl hash_db::Hasher for Keccak256Hasher {
		type Out = H256;
		type StdHasher = plain_hasher::PlainHasher;
		const LENGTH: usize = 32;
		fn hash(x: &[u8]) -> Self::Out {
			keccak_256(x).into()
		}
	}

	triehash::ordered_trie_root::<Keccak256Hasher, _>(items)
}

/// Get validator that should author the block at given step.
pub fn step_validator<T>(header_validators: &[T], header_step: u64) -> &T {
	&header_validators[(header_step % header_validators.len() as u64) as usize]
}

sp_api::decl_runtime_apis! {
	/// API for headers submitters.
	pub trait EthereumHeadersApi {
		/// Returns number and hash of the best block known to the bridge module.
		/// The caller should only submit `import_header` transaction that makes
		/// (or leads to making) other header the best one.
		fn best_block() -> (u64, H256);

		/// Returns true if the import of given block requires transactions receipts.
		fn is_import_requires_receipts(header: Header) -> bool;

		/// Returns true if header is known to the runtime.
		fn is_known_block(hash: H256) -> bool;
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use hex_literal::hex;

	#[test]
	fn transfer_transaction_decode_works() {
		// value transfer transaction
		// https://etherscan.io/tx/0xb9d4ad5408f53eac8627f9ccd840ba8fb3469d55cd9cc2a11c6e049f1eef4edd
		// https://etherscan.io/getRawTx?tx=0xb9d4ad5408f53eac8627f9ccd840ba8fb3469d55cd9cc2a11c6e049f1eef4edd
		let raw_tx = hex!("f86c0a85046c7cfe0083016dea94d1310c1e038bc12865d3d3997275b3e4737c6302880b503be34d9fe80080269fc7eaaa9c21f59adf8ad43ed66cf5ef9ee1c317bd4d32cd65401e7aaca47cfaa0387d79c65b90be6260d09dcfb780f29dd8133b9b1ceb20b83b7e442b4bfc30cb");
		assert_eq!(
			transaction_decode(&raw_tx),
			Ok(Transaction {
				sender: hex!("67835910d32600471f388a137bbff3eb07993c04").into(),
				unsigned: UnsignedTransaction {
					nonce: 10.into(),
					gas_price: 19000000000u64.into(),
					gas: 93674.into(),
					to: Some(hex!("d1310c1e038bc12865d3d3997275b3e4737c6302").into()),
					value: 815217380000000000_u64.into(),
					payload: Default::default(),
				}
			}),
		);

		// Kovan value transfer transaction
		// https://kovan.etherscan.io/tx/0x3b4b7bd41c1178045ccb4753aa84c1ef9864b4d712fa308b228917cd837915da
		// https://kovan.etherscan.io/getRawTx?tx=0x3b4b7bd41c1178045ccb4753aa84c1ef9864b4d712fa308b228917cd837915da
		let raw_tx = hex!("f86a822816808252089470c1ccde719d6f477084f07e4137ab0e55f8369f8930cf46e92063afd8008078a00e4d1f4d8aa992bda3c105ff3d6e9b9acbfd99facea00985e2131029290adbdca028ea29a46a4b66ec65b454f0706228e3768cb0ecf755f67c50ddd472f11d5994");
		assert_eq!(
			transaction_decode(&raw_tx),
			Ok(Transaction {
				sender: hex!("faadface3fbd81ce37b0e19c0b65ff4234148132").into(),
				unsigned: UnsignedTransaction {
					nonce: 10262.into(),
					gas_price: 0.into(),
					gas: 21000.into(),
					to: Some(hex!("70c1ccde719d6f477084f07e4137ab0e55f8369f").into()),
					value: 900379597077600000000_u128.into(),
					payload: Default::default(),
				},
			}),
		);
	}

	#[test]
	fn payload_transaction_decode_works() {
		// contract call transaction
		// https://etherscan.io/tx/0xdc2b996b4d1d6922bf6dba063bfd70913279cb6170967c9bb80252aeb061cf65
		// https://etherscan.io/getRawTx?tx=0xdc2b996b4d1d6922bf6dba063bfd70913279cb6170967c9bb80252aeb061cf65
		let raw_tx = hex!("f8aa76850430e234008301500094dac17f958d2ee523a2206206994597c13d831ec780b844a9059cbb000000000000000000000000e08f35f66867a454835b25118f1e490e7f9e9a7400000000000000000000000000000000000000000000000000000000004c4b4025a0964e023999621dc3d4d831c43c71f7555beb6d1192dee81a3674b3f57e310f21a00f229edd86f841d1ee4dc48cc16667e2283817b1d39bae16ced10cd206ae4fd4");
		assert_eq!(
			transaction_decode(&raw_tx),
			Ok(Transaction {
				sender: hex!("2b9a4d37bdeecdf994c4c9ad7f3cf8dc632f7d70").into(),
				unsigned: UnsignedTransaction {
					nonce: 118.into(),
					gas_price: 18000000000u64.into(),
					gas: 86016.into(),
					to: Some(hex!("dac17f958d2ee523a2206206994597c13d831ec7").into()),
					value: 0.into(),
					payload: hex!("a9059cbb000000000000000000000000e08f35f66867a454835b25118f1e490e7f9e9a7400000000000000000000000000000000000000000000000000000000004c4b40").to_vec().into(),
				},
			}),
		);

		// Kovan contract call transaction
		// https://kovan.etherscan.io/tx/0x2904b4451d23665492239016b78da052d40d55fdebc7304b38e53cf6a37322cf
		// https://kovan.etherscan.io/getRawTx?tx=0x2904b4451d23665492239016b78da052d40d55fdebc7304b38e53cf6a37322cf
		let raw_tx = hex!("f8ac8302200b843b9aca00830271009484dd11eb2a29615303d18149c0dbfa24167f896680b844a9059cbb00000000000000000000000001503dfc5ad81bf630d83697e98601871bb211b600000000000000000000000000000000000000000000000000000000000027101ba0ce126d2cca81f5e245f292ff84a0d915c0a4ac52af5c51219db1e5d36aa8da35a0045298b79dac631907403888f9b04c2ab5509fe0cc31785276d30a40b915fcf9");
		assert_eq!(
			transaction_decode(&raw_tx),
			Ok(Transaction {
				sender: hex!("617da121abf03d4c1af572f5a4e313e26bef7bdc").into(),
				unsigned: UnsignedTransaction {
					nonce: 139275.into(),
					gas_price: 1000000000.into(),
					gas: 160000.into(),
					to: Some(hex!("84dd11eb2a29615303d18149c0dbfa24167f8966").into()),
					value: 0.into(),
					payload: hex!("a9059cbb00000000000000000000000001503dfc5ad81bf630d83697e98601871bb211b60000000000000000000000000000000000000000000000000000000000002710").to_vec().into(),
				},
			}),
		);
	}
}
