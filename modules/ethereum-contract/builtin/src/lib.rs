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

use bridge_node_runtime::{Block, BlockNumber, Hash, Header as RuntimeHeader};
use codec::{Decode, Encode};
use ethereum_types::U256;
use sp_blockchain::Error as ClientError;
use sp_finality_grandpa::{AuthorityList, ConsensusLog, GRANDPA_ENGINE_ID};

/// Builtin errors.
#[derive(Debug)]
pub enum Error {
	/// Failed to decode block number.
	BlockNumberDecode,
	/// Failed to decode Substrate header.
	HeaderDecode(codec::Error),
	/// Failed to decode best voters set.
	BestSetDecode(codec::Error),
	/// Failed to decode finality proof.
	FinalityProofDecode(codec::Error),
	/// Failed to verify justification.
	JustificationVerify(ClientError),
}

/// Substrate header.
#[derive(Debug, PartialEq)]
pub struct Header {
	/// Header hash.
	pub hash: Hash,
	/// Parent header hash.
	pub parent_hash: Hash,
	/// Header number.
	pub number: BlockNumber,
	/// GRANDPA validators change signal.
	pub signal: Option<ValidatorsSetSignal>,
}

/// GRANDPA validators set change signal.
#[derive(Debug, PartialEq)]
pub struct ValidatorsSetSignal {
	/// Signal delay.
	pub delay: BlockNumber,
	/// New validators set.
	pub validators: Vec<u8>,
}

/// Convert from U256 to BlockNumber.
pub fn to_substrate_block_number(number: U256) -> Result<BlockNumber, Error> {
	match number == number.low_u32().into() {
		true => Ok(number.low_u32()),
		false => Err(Error::BlockNumberDecode),
	}
}

/// Convert from BlockNumber to U256.
pub fn from_substrate_block_number(number: BlockNumber) -> Result<U256, Error> {
	Ok(U256::from(number as u64))
}

/// Parse Substrate header.
pub fn parse_substrate_header(raw_header: &[u8]) -> Result<Header, Error> {
	RuntimeHeader::decode(&mut &raw_header[..])
		.map(|header| Header {
			hash: header.hash(),
			parent_hash: header.parent_hash,
			number: header.number,
			signal: sp_runtime::traits::Header::digest(&header)
				.log(|log| {
					log.as_consensus().and_then(|(engine_id, log)| {
						if engine_id == GRANDPA_ENGINE_ID {
							Some(log)
						} else {
							None
						}
					})
				})
				.and_then(|log| ConsensusLog::decode(&mut &log[..]).ok())
				.and_then(|log| match log {
					ConsensusLog::ScheduledChange(scheduled_change) => Some(ValidatorsSetSignal {
						delay: scheduled_change.delay,
						validators: scheduled_change.next_authorities.encode(),
					}),
					_ => None,
				}),
		})
		.map_err(Error::HeaderDecode)
}

/// Verify GRANDPA finality proof.
pub fn verify_substrate_finality_proof(
	finality_target_number: BlockNumber,
	finality_target_hash: Hash,
	best_set_id: u64,
	raw_best_set: &[u8],
	raw_finality_proof: &[u8],
) -> Result<(), Error> {
	let best_set = AuthorityList::decode(&mut &raw_best_set[..]).map_err(Error::BestSetDecode)?;
	sc_finality_grandpa::GrandpaJustification::<Block>::decode_and_verify_finalizes(
		&raw_finality_proof,
		(finality_target_hash, finality_target_number),
		best_set_id,
		&best_set.into_iter().collect(),
	)
	.map_err(Error::JustificationVerify)
	.map(|_| ())
}

#[cfg(test)]
mod tests {
	use hex_literal::hex;
	use super::*;

	#[test]
	fn to_substrate_block_number_succeeds() {
		assert_eq!(to_substrate_block_number(U256::zero()).unwrap(), 0);
		assert_eq!(to_substrate_block_number(U256::from(std::u32::MAX as u64)).unwrap(), 0xFFFFFFFF);
	}

	#[test]
	fn to_substrate_block_number_fails() {
		assert!(matches!(
			to_substrate_block_number(U256::from(std::u32::MAX as u64 + 1)),
			Err(Error::BlockNumberDecode)
		));
	}

	#[test]
	fn from_substrate_block_number_succeeds() {
		assert_eq!(from_substrate_block_number(0).unwrap(), U256::zero());
		assert_eq!(from_substrate_block_number(std::u32::MAX).unwrap(), U256::from(std::u32::MAX));
	}

	#[test]
	fn substrate_header_without_signal_parsed() {
		assert_eq!(
			parse_substrate_header(&hex!("000000000000000000000000000000000000000000000000000000000000000000b2fc47904df5e355c6ab476d89fbc0733aeddbe302f0b94ba4eea9283f7e89e703170a2e7597b7b7e3d84c05391d139a62b157e78786d8c082f29dcf4c11131400")).unwrap(),
			Header {
				hash: "afbbeb92bf6ff14f60bdef0aa89f043dd403659ae82665238810ace0d761f6d0".parse().unwrap(),
				parent_hash: Default::default(),
				number: 0,
				signal: None,
			},
		);
	}

	#[test]
	fn substrate_header_with_signal_parsed() {
		assert_eq!(
			parse_substrate_header(&hex!("c0ac300d4005141ea690f3df593e049739c227316eb7f05052f3ee077388b68b20822d6b412033aa9ac8e1722918eec5f25633529225754b3d4149982f5cacd4aae7b07c0ce2799416ce7877b9cefc7f596bea5e8813bb2a0abf760414073ca92810066175726120ae54c70f0000000004617572618901010c90b5ab205c6974c9ea841be688864633dc9ca8a357843eeacf2314649965fe22306721211d5404bd9da88e0204360a1a9ab8b87c66c1bc2fcdd37f3c2222cc20e659a7a1628cdd93febc04a4e0646ea20e9f5f0ce097d9a05290d4a9e054df4e0446524e4bf901010c439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234f01000000000000005e639b43e0052c47447dac87d6fd2b6ec50bdd4d0f614e4299c665249bbd09d901000000000000001dfe3e22cc0d45c70779c1095f7489a8ef3cf52d62fbd8c2fa38c9f1723502b501000000000000000000000005617572610101acd5bb6f958101f460c5f86a1c45f6a28bc66fa436508e3db597ba76edefa5552c09b76850fd34328fa938b591cc1d59445d713b4ab59c750f5cdccd259d0383")).unwrap(),
			Header {
				hash: "4444c880757dc15aa0aa54b26a7738131ffb33a95fc2b7cea1c1f5e933157ebd".parse().unwrap(),
				parent_hash: "c0ac300d4005141ea690f3df593e049739c227316eb7f05052f3ee077388b68b".parse().unwrap(),
				number: 8,
				signal: Some(ValidatorsSetSignal {
					delay: 0,
					validators: hex!("0c439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234f01000000000000005e639b43e0052c47447dac87d6fd2b6ec50bdd4d0f614e4299c665249bbd09d901000000000000001dfe3e22cc0d45c70779c1095f7489a8ef3cf52d62fbd8c2fa38c9f1723502b50100000000000000").to_vec(),
				}),
			},
		);
	}

	#[test]
	fn substrate_header_parse_fails() {
		assert!(matches!(
			parse_substrate_header(&[]),
			Err(_)
		));
	}

	#[test]
	fn verify_substrate_finality_proof_succeeds() {
		verify_substrate_finality_proof(
			8,
			"a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f343775".parse().unwrap(),
			0,
			&hex!("1488dc3417d5058ec4b4503e0c12ea1a0a89be200fe98922423d4334014fa6b0ee0100000000000000d17c2d7823ebf260fd138f2d7e27d114c0145d968b5ff5006125f2414fadae690100000000000000439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234f01000000000000005e639b43e0052c47447dac87d6fd2b6ec50bdd4d0f614e4299c665249bbd09d901000000000000001dfe3e22cc0d45c70779c1095f7489a8ef3cf52d62fbd8c2fa38c9f1723502b50100000000000000"),
			&hex!("2600000000000000a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f3437750800000010a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000d66b4ceb57ef8bcbc955071b597c8c5d2adcfdbb009c73f8438d342670fdeca9ac60686cbd58105b10f51d0a64a8e73b2e5829b2eab3248a008c472852130b00439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234fa2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000f5730c14d3cd22b7661e2f5fcb3139dd5fef37f946314a441d01b40ce1200ef70d810525f23fd278b588cd67473c200bda83c338c407b479386aa83798e5970b5e639b43e0052c47447dac87d6fd2b6ec50bdd4d0f614e4299c665249bbd09d9a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000c78d6ec463f476461a695b4791d30e7626d16fdf72d7c252c2cad387495a97e8c2827ed4d5af853d6e05d31cb6fb7438c9481a7e9c6990d60a9bfaf6a6e1930988dc3417d5058ec4b4503e0c12ea1a0a89be200fe98922423d4334014fa6b0eea2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f3437750800000052b4fc52d430286b3e2d650aa6e01b6ff4fae8b968893a62be789209eb97ee6e23780d3f5af7042d85bb48f1b202890b22724dfebce138826f66a5e00324320fd17c2d7823ebf260fd138f2d7e27d114c0145d968b5ff5006125f2414fadae6900"),
		).unwrap();
	}

	#[test]
	fn verify_substrate_finality_proof_fails_when_wrong_block_is_finalized() {
		verify_substrate_finality_proof(
			4,
			Default::default(),
			0,
			&hex!("1488dc3417d5058ec4b4503e0c12ea1a0a89be200fe98922423d4334014fa6b0ee0100000000000000d17c2d7823ebf260fd138f2d7e27d114c0145d968b5ff5006125f2414fadae690100000000000000439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234f01000000000000005e639b43e0052c47447dac87d6fd2b6ec50bdd4d0f614e4299c665249bbd09d901000000000000001dfe3e22cc0d45c70779c1095f7489a8ef3cf52d62fbd8c2fa38c9f1723502b50100000000000000"),
			&hex!("2600000000000000a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f3437750800000010a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000d66b4ceb57ef8bcbc955071b597c8c5d2adcfdbb009c73f8438d342670fdeca9ac60686cbd58105b10f51d0a64a8e73b2e5829b2eab3248a008c472852130b00439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234fa2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000f5730c14d3cd22b7661e2f5fcb3139dd5fef37f946314a441d01b40ce1200ef70d810525f23fd278b588cd67473c200bda83c338c407b479386aa83798e5970b5e639b43e0052c47447dac87d6fd2b6ec50bdd4d0f614e4299c665249bbd09d9a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000c78d6ec463f476461a695b4791d30e7626d16fdf72d7c252c2cad387495a97e8c2827ed4d5af853d6e05d31cb6fb7438c9481a7e9c6990d60a9bfaf6a6e1930988dc3417d5058ec4b4503e0c12ea1a0a89be200fe98922423d4334014fa6b0eea2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f3437750800000052b4fc52d430286b3e2d650aa6e01b6ff4fae8b968893a62be789209eb97ee6e23780d3f5af7042d85bb48f1b202890b22724dfebce138826f66a5e00324320fd17c2d7823ebf260fd138f2d7e27d114c0145d968b5ff5006125f2414fadae6900"),
		).unwrap_err();
	}

	#[test]
	fn verify_substrate_finality_proof_fails_when_wrong_set_is_provided() {
		verify_substrate_finality_proof(
			8,
			"a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f343775".parse().unwrap(),
			0,
			&hex!("1488dc3417d5058ec4b4503e0c12ea"),
			&hex!("2600000000000000a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f3437750800000010a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000d66b4ceb57ef8bcbc955071b597c8c5d2adcfdbb009c73f8438d342670fdeca9ac60686cbd58105b10f51d0a64a8e73b2e5829b2eab3248a008c472852130b00439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234fa2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000f5730c14d3cd22b7661e2f5fcb3139dd5fef37f946314a441d01b40ce1200ef70d810525f23fd278b588cd67473c200bda83c338c407b479386aa83798e5970b5e639b43e0052c47447dac87d6fd2b6ec50bdd4d0f614e4299c665249bbd09d9a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000c78d6ec463f476461a695b4791d30e7626d16fdf72d7c252c2cad387495a97e8c2827ed4d5af853d6e05d31cb6fb7438c9481a7e9c6990d60a9bfaf6a6e1930988dc3417d5058ec4b4503e0c12ea1a0a89be200fe98922423d4334014fa6b0eea2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f3437750800000052b4fc52d430286b3e2d650aa6e01b6ff4fae8b968893a62be789209eb97ee6e23780d3f5af7042d85bb48f1b202890b22724dfebce138826f66a5e00324320fd17c2d7823ebf260fd138f2d7e27d114c0145d968b5ff5006125f2414fadae6900"),
		).unwrap_err();
	}

	#[test]
	fn verify_substrate_finality_proof_fails_when_wrong_set_id_is_provided() {
		verify_substrate_finality_proof(
			8,
			"a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f343775".parse().unwrap(),
			42,
			&hex!("1488dc3417d5058ec4b4503e0c12ea1a0a89be200fe98922423d4334014fa6b0ee0100000000000000d17c2d7823ebf260fd138f2d7e27d114c0145d968b5ff5006125f2414fadae690100000000000000439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234f01000000000000005e639b43e0052c47447dac87d6fd2b6ec50bdd4d0f614e4299c665249bbd09d901000000000000001dfe3e22cc0d45c70779c1095f7489a8ef3cf52d62fbd8c2fa38c9f1723502b50100000000000000"),
			&hex!("2600000000000000a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f3437750800000010a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000d66b4ceb57ef8bcbc955071b597c8c5d2adcfdbb009c73f8438d342670fdeca9ac60686cbd58105b10f51d0a64a8e73b2e5829b2eab3248a008c472852130b00439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234fa2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000f5730c14d3cd22b7661e2f5fcb3139dd5fef37f946314a441d01b40ce1200ef70d810525f23fd278b588cd67473c200bda83c338c407b479386aa83798e5970b5e639b43e0052c47447dac87d6fd2b6ec50bdd4d0f614e4299c665249bbd09d9a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f34377508000000c78d6ec463f476461a695b4791d30e7626d16fdf72d7c252c2cad387495a97e8c2827ed4d5af853d6e05d31cb6fb7438c9481a7e9c6990d60a9bfaf6a6e1930988dc3417d5058ec4b4503e0c12ea1a0a89be200fe98922423d4334014fa6b0eea2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f3437750800000052b4fc52d430286b3e2d650aa6e01b6ff4fae8b968893a62be789209eb97ee6e23780d3f5af7042d85bb48f1b202890b22724dfebce138826f66a5e00324320fd17c2d7823ebf260fd138f2d7e27d114c0145d968b5ff5006125f2414fadae6900"),
		).unwrap_err();

	}

	#[test]
	fn verify_substrate_finality_proof_fails_when_wrong_proof_is_provided() {
		verify_substrate_finality_proof(
			8,
			"a2f45892db86b2ad133ce57d81b7e4375bb7035ce9883e6b68c358164f343775".parse().unwrap(),
			0,
			&hex!("1488dc3417d5058ec4b4503e0c12ea1a0a89be200fe98922423d4334014fa6b0ee0100000000000000d17c2d7823ebf260fd138f2d7e27d114c0145d968b5ff5006125f2414fadae690100000000000000439660b36c6c03afafca027b910b4fecf99801834c62a5e6006f27d978de234f01000000000000005e639b43e0052c47447dac87d6fd2b6ec50bdd4d0f614e4299c665249bbd09d901000000000000001dfe3e22cc0d45c70779c1095f7489a8ef3cf52d62fbd8c2fa38c9f1723502b50100000000000000"),
			&hex!("2600000000000000a2f45892"),
		).unwrap_err();
	}
}
