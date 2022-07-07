// Copyright 2019-2022 Parity Technologies (UK) Ltd.
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

use async_trait::async_trait;
use std::sync::Arc;

use crate::cli::{
	bridge::{CliBridgeBase, MessagesCliBridge, RelayHeadersCliBridge},
	chain_schema::*,
	relay_headers_and_messages::{Full2WayBridgeBase, Full2WayBridgeCommonParams},
	CliChain,
};
use bp_runtime::BlockNumberOf;
use relay_substrate_client::{AccountIdOf, AccountKeyPairOf, Chain, Client, TransactionSignScheme};
use sp_core::Pair;
use substrate_relay_helper::{
	finality::SubstrateFinalitySyncPipeline,
	on_demand::{headers::OnDemandHeadersRelay, OnDemandRelay},
	TaggedAccount,
};

pub struct ChainToChainBridgeParams {
	pub common: Full2WayBridgeCommonParams,
	// override for right->left headers signer
	pub right_headers_to_left_sign_override: SigningParams,
	// override for left->right headers signer
	pub left_headers_to_right_sign_override: SigningParams,
}

macro_rules! declare_chain_to_chain_bridge_schema {
	($left_chain:ident, $right_chain:ident) => {
		paste::item! {
			#[doc = $left_chain " and " $right_chain " headers+messages relay params."]
			#[derive(Debug, PartialEq, StructOpt)]
			pub struct [<$left_chain $right_chain HeadersAndMessages>] {
				#[structopt(flatten)]
				shared: HeadersAndMessagesSharedParams,
				// default signer, which is always used to sign messages relay transactions on the left chain
				#[structopt(flatten)]
				left: [<$left_chain ConnectionParams>],
				// override for right->left headers signer
				#[structopt(flatten)]
				right_headers_to_left_sign_override: [<$right_chain HeadersTo $left_chain SigningParams>],
				#[structopt(flatten)]
				left_sign: [<$left_chain SigningParams>],
				#[structopt(flatten)]
				left_messages_pallet_owner: [<$left_chain MessagesPalletOwnerSigningParams>],
				// default signer, which is always used to sign messages relay transactions on the right chain
				#[structopt(flatten)]
				right: [<$right_chain ConnectionParams>],
				// override for left->right headers signer
				#[structopt(flatten)]
				left_headers_to_right_sign_override: [<$left_chain HeadersTo $right_chain SigningParams>],
				#[structopt(flatten)]
				right_sign: [<$right_chain SigningParams>],
				#[structopt(flatten)]
				right_messages_pallet_owner: [<$right_chain MessagesPalletOwnerSigningParams>],
			}

			impl From<[<$left_chain $right_chain HeadersAndMessages>]> for ChainToChainBridgeParams {
				fn from(item: [<$left_chain $right_chain HeadersAndMessages>]) -> ChainToChainBridgeParams {
					ChainToChainBridgeParams {
						common: Full2WayBridgeCommonParams {
							shared: item.shared,
							left: item.left.into(),
							left_sign: item.left_sign.into(),
							left_messages_pallet_owner: item.left_messages_pallet_owner.into(),
							right: item.right.into(),
							right_sign: item.right_sign.into(),
							right_messages_pallet_owner: item.right_messages_pallet_owner.into(),
						},
						right_headers_to_left_sign_override: item.right_headers_to_left_sign_override.into(),
						left_headers_to_right_sign_override: item.left_headers_to_right_sign_override.into(),
					}
				}
			}
		}
	};
}

pub struct ChainToChainEnv<
	L2R: MessagesCliBridge + RelayHeadersCliBridge,
	R2L: MessagesCliBridge + RelayHeadersCliBridge,
> {
	params: ChainToChainBridgeParams,

	left_client: Client<<L2R as CliBridgeBase>::Source>,
	right_client: Client<<R2L as CliBridgeBase>::Source>,

	at_left_accounts: Vec<TaggedAccount<AccountIdOf<<L2R as CliBridgeBase>::Source>>>,
	at_right_accounts: Vec<TaggedAccount<AccountIdOf<<R2L as CliBridgeBase>::Source>>>,
}

#[async_trait]
impl<
		Left: Chain + TransactionSignScheme<Chain = Left> + CliChain<KeyPair = AccountKeyPairOf<Left>>,
		Right: Chain + TransactionSignScheme<Chain = Right> + CliChain<KeyPair = AccountKeyPairOf<Right>>,
		L2R: CliBridgeBase<Source = Left, Target = Right> + MessagesCliBridge + RelayHeadersCliBridge,
		R2L: CliBridgeBase<Source = Right, Target = Left> + MessagesCliBridge + RelayHeadersCliBridge,
	> Full2WayBridgeBase for ChainToChainEnv<L2R, R2L>
where
	AccountIdOf<Left>: From<<AccountKeyPairOf<Left> as Pair>::Public>,
	AccountIdOf<Right>: From<<AccountKeyPairOf<Right> as Pair>::Public>,
{
	type Params = ChainToChainBridgeParams;
	type Left = Left;
	type Right = Right;

	async fn new(params: ChainToChainBridgeParams) -> anyhow::Result<Self> {
		let left_client = params.common.left.to_client::<Left>().await?;
		let right_client = params.common.right.to_client::<Right>().await?;

		Ok(Self {
			params,
			left_client,
			right_client,
			at_left_accounts: vec![],
			at_right_accounts: vec![],
		})
	}

	fn common(&self) -> &Full2WayBridgeCommonParams {
		&self.params.common
	}

	fn left_client(&self) -> &Client<Self::Left> {
		&self.left_client
	}

	fn right_client(&self) -> &Client<Self::Right> {
		&self.right_client
	}

	fn mut_at_left_accounts(&mut self) -> &mut Vec<TaggedAccount<AccountIdOf<Left>>> {
		&mut self.at_left_accounts
	}

	fn mut_at_right_accounts(&mut self) -> &mut Vec<TaggedAccount<AccountIdOf<Right>>> {
		&mut self.at_right_accounts
	}

	async fn start_on_demand_headers_relayers(
		&mut self,
	) -> anyhow::Result<(
		Arc<dyn OnDemandRelay<BlockNumberOf<Self::Left>>>,
		Arc<dyn OnDemandRelay<BlockNumberOf<Self::Right>>>,
	)> {
		let left_to_right_transaction_params = self
			.params
			.left_headers_to_right_sign_override
			.transaction_params_or::<<L2R as CliBridgeBase>::Target, _>(
			&self.params.common.right_sign,
		)?;
		let right_to_left_transaction_params =
			self.params
				.right_headers_to_left_sign_override
				.transaction_params_or::<Self::Left, _>(&self.params.common.left_sign)?;

		self.mut_at_right_accounts().push(TaggedAccount::Headers {
			id: left_to_right_transaction_params.signer.public().into(),
			bridged_chain: Self::Left::NAME.to_string(),
		});
		self.mut_at_left_accounts().push(TaggedAccount::Headers {
			id: right_to_left_transaction_params.signer.public().into(),
			bridged_chain: Self::Right::NAME.to_string(),
		});

		<L2R as RelayHeadersCliBridge>::Finality::start_relay_guards(
			&self.right_client,
			&left_to_right_transaction_params,
			self.params.common.right.can_start_version_guard(),
		)
		.await?;
		<R2L as RelayHeadersCliBridge>::Finality::start_relay_guards(
			&self.left_client,
			&right_to_left_transaction_params,
			self.params.common.left.can_start_version_guard(),
		)
		.await?;

		let left_to_right_on_demand_headers =
			OnDemandHeadersRelay::new::<<L2R as RelayHeadersCliBridge>::Finality>(
				self.left_client.clone(),
				self.right_client.clone(),
				left_to_right_transaction_params,
				self.params.common.shared.only_mandatory_headers,
			);
		let right_to_left_on_demand_headers =
			OnDemandHeadersRelay::new::<<R2L as RelayHeadersCliBridge>::Finality>(
				self.right_client.clone(),
				self.left_client.clone(),
				right_to_left_transaction_params,
				self.params.common.shared.only_mandatory_headers,
			);

		Ok((Arc::new(left_to_right_on_demand_headers), Arc::new(right_to_left_on_demand_headers)))
	}
}
