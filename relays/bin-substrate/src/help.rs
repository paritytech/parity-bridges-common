// // Copyright 2019-2021 Parity Technologies (UK) Ltd.
// // This file is part of Parity Bridges Common.
//
// // Parity Bridges Common is free software: you can redistribute it and/or modify
// // it under the terms of the GNU General Public License as published by
// // the Free Software Foundation, either version 3 of the License, or
// // (at your option) any later version.
//
// // Parity Bridges Common is distributed in the hope that it will be useful,
// // but WITHOUT ANY WARRANTY; without even the implied warranty of
// // MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// // GNU General Public License for more details.
//
// // You should have received a copy of the GNU General Public License
// // along with Parity Bridges Common.  If not, see <http://www.gnu.org/licenses/>.
//
// use bp_header_chain::justification::GrandpaJustification;
// use finality_relay::FinalitySyncPipeline;
// use messages_relay::message_lane::MessageLane;
// use relay_substrate_client::{
// 	metrics::{FloatStorageValueMetric, StorageProofOverheadMetric},
// 	BlockNumberOf, Chain, Client, HashOf, SyncHeader,
// };
// use relay_utils::BlockNumberBase;
// use std::fmt::Debug;
// use std::marker::PhantomData;
// use substrate_relay_helper::messages_source::SubstrateMessagesProof;
// use substrate_relay_helper::messages_target::SubstrateMessagesReceivingProof;
//
// /// Substrate-to-Substrate message lane.
// #[derive(Debug)]
// pub struct SubstrateMessageLaneToSubstrate<Source: Chain, SourceSignParams, Target: Chain, TargetSignParams> {
// 	/// Client for the source Substrate chain.
// 	pub(crate) source_client: Client<Source>,
// 	/// Parameters required to sign transactions for source chain.
// 	pub(crate) source_sign: SourceSignParams,
// 	/// Client for the target Substrate chain.
// 	pub(crate) target_client: Client<Target>,
// 	/// Parameters required to sign transactions for target chain.
// 	pub(crate) target_sign: TargetSignParams,
// 	/// Account id of relayer at the source chain.
// 	pub(crate) relayer_id_at_source: Source::AccountId,
// }
//
// impl<Source: Chain, SourceSignParams: Clone, Target: Chain, TargetSignParams: Clone> Clone
// 	for SubstrateMessageLaneToSubstrate<Source, SourceSignParams, Target, TargetSignParams>
// {
// 	fn clone(&self) -> Self {
// 		Self {
// 			source_client: self.source_client.clone(),
// 			source_sign: self.source_sign.clone(),
// 			target_client: self.target_client.clone(),
// 			target_sign: self.target_sign.clone(),
// 			relayer_id_at_source: self.relayer_id_at_source.clone(),
// 		}
// 	}
// }
//
// impl<Source: Chain, SourceSignParams, Target: Chain, TargetSignParams> MessageLane
// 	for SubstrateMessageLaneToSubstrate<Source, SourceSignParams, Target, TargetSignParams>
// where
// 	SourceSignParams: Clone + Send + Sync + 'static,
// 	TargetSignParams: Clone + Send + Sync + 'static,
// 	BlockNumberOf<Source>: BlockNumberBase,
// 	BlockNumberOf<Target>: BlockNumberBase,
// {
// 	const SOURCE_NAME: &'static str = Source::NAME;
// 	const TARGET_NAME: &'static str = Target::NAME;
//
// 	type MessagesProof = SubstrateMessagesProof<Source>;
// 	type MessagesReceivingProof = SubstrateMessagesReceivingProof<Target>;
//
// 	type SourceChainBalance = Source::Balance;
// 	type SourceHeaderNumber = BlockNumberOf<Source>;
// 	type SourceHeaderHash = HashOf<Source>;
//
// 	type TargetHeaderNumber = BlockNumberOf<Target>;
// 	type TargetHeaderHash = HashOf<Target>;
// }
//
// /// Substrate-to-Substrate finality proof pipeline.
// #[derive(Clone)]
// pub struct SubstrateFinalityToSubstrate<SourceChain, TargetChain: Chain, TargetSign> {
// 	/// Client for the target chain.
// 	pub(crate) target_client: Client<TargetChain>,
// 	/// Data required to sign target chain transactions.
// 	pub(crate) target_sign: TargetSign,
// 	/// Unused generic arguments dump.
// 	_marker: PhantomData<SourceChain>,
// }
//
// impl<SourceChain, TargetChain: Chain, TargetSign> Debug
// 	for SubstrateFinalityToSubstrate<SourceChain, TargetChain, TargetSign>
// {
// 	fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
// 		f.debug_struct("SubstrateFinalityToSubstrate")
// 			.field("target_client", &self.target_client)
// 			.finish()
// 	}
// }
//
// impl<SourceChain, TargetChain: Chain, TargetSign> SubstrateFinalityToSubstrate<SourceChain, TargetChain, TargetSign> {
// 	/// Create new Substrate-to-Substrate headers pipeline.
// 	pub fn new(target_client: Client<TargetChain>, target_sign: TargetSign) -> Self {
// 		SubstrateFinalityToSubstrate {
// 			target_client,
// 			target_sign,
// 			_marker: Default::default(),
// 		}
// 	}
// }
//
// impl<SourceChain, TargetChain, TargetSign> FinalitySyncPipeline
// 	for SubstrateFinalityToSubstrate<SourceChain, TargetChain, TargetSign>
// where
// 	SourceChain: Clone + Chain + Debug,
// 	BlockNumberOf<SourceChain>: BlockNumberBase,
// 	TargetChain: Clone + Chain + Debug,
// 	TargetSign: 'static + Clone + Send + Sync,
// {
// 	const SOURCE_NAME: &'static str = SourceChain::NAME;
// 	const TARGET_NAME: &'static str = TargetChain::NAME;
//
// 	type Hash = HashOf<SourceChain>;
// 	type Number = BlockNumberOf<SourceChain>;
// 	type Header = SyncHeader<SourceChain::Header>;
// 	type FinalityProof = GrandpaJustification<SourceChain::Header>;
// }
