// Copyright 2019-2021 Parity Technologies (UK) Ltd.
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

//! Tokens swap using token-swap bridge pallet.

use codec::Encode;
use num_traits::{One, Zero};
use rand::random;
use structopt::StructOpt;
use strum::{EnumString, EnumVariantNames, VariantNames};

use frame_support::dispatch::GetDispatchInfo;
use relay_substrate_client::{
	AccountIdOf, AccountPublicOf, BalanceOf, BlockNumberOf, BlockWithJustification, CallOf, Chain, ChainWithBalances,
	Client, Error as SubstrateError, HashOf, Subscription, TransactionSignScheme, TransactionStatusOf,
	UnsignedTransaction,
};
use sp_core::{Bytes, H256, Hasher, Pair, U256, blake2_256, storage::StorageKey};
use sp_runtime::traits::{Convert, Header as HeaderT};

use crate::cli::{Balance, SourceConnectionParams, SourceSigningParams, TargetConnectionParams, TargetSigningParams};

/// Swap tokens.
#[derive(StructOpt)]
pub struct SwapTokens {
	/// A bridge instance to use in token swap.
	#[structopt(possible_values = SwapTokensBridge::VARIANTS, case_insensitive = true)]
	bridge: SwapTokensBridge,

	#[structopt(flatten)]
	source: SourceConnectionParams,
	#[structopt(flatten)]
	source_sign: SourceSigningParams,

	#[structopt(flatten)]
	target: TargetConnectionParams,
	#[structopt(flatten)]
	target_sign: TargetSigningParams,

	#[structopt(subcommand)]
	swap_type: TokenSwapType,
	/// Source chain balance that source signer wants to swap.
	#[structopt(long)]
	source_balance: Balance,
	/// Target chain balance that target signer wants to swap.
	#[structopt(long)]
	target_balance: Balance,
}

/// Token swap type.
#[derive(StructOpt, Debug, PartialEq, Eq)]
pub enum TokenSwapType {
	/// The `target_sign` is temporary and only have funds for single swap.
	TemporaryTargetAccountAtBridgedChain,
	/// This swap type prevents `source_signer` from restarting the swap after it has been completed.
	LockClaimUntilBlock {
		/// Number of blocks before the swap expires.
		#[structopt(long)]
		blocks_before_expire: u32,
		/// Unique swap nonce.
		#[structopt(long)]
		swap_nonce: Option<U256>,
	}
}

/// Swap tokens bridge.
#[derive(Debug, EnumString, EnumVariantNames)]
#[strum(serialize_all = "kebab_case")]
pub enum SwapTokensBridge {
	/// Use token-swap pallet deployed at Millau to swap tokens with Rialto.
	MillauToRialto,
}

macro_rules! select_bridge {
	($bridge: expr, $generic: tt) => {
		match $bridge {
			SwapTokensBridge::MillauToRialto => {
				type Source = relay_millau_client::Millau;
				type Target = relay_rialto_client::Rialto;

				//type FromTargetToSourceAccountIdConverter = bp_millau::AccountIdConverter;
				//type FromSourceToTargetAccountIdConverter = bp_rialto::AccountIdConverter;
				type FromSwapToThisAccountIdConverter = bp_rialto::AccountIdConverter;

				use bp_millau::{
					derive_account_from_rialto_id as derive_source_account_from_target_account,
					WITH_RIALTO_TOKEN_SWAP_PALLET_NAME as TOKEN_SWAP_PALLET_NAME,
				};
				use bp_rialto::derive_account_from_millau_id as derive_target_account_from_source_account;

				const SOURCE_CHAIN_ID: bp_runtime::ChainId = bp_runtime::MILLAU_CHAIN_ID;
				const TARGET_CHAIN_ID: bp_runtime::ChainId = bp_runtime::RIALTO_CHAIN_ID;

				const SOURCE_SPEC_VERSION: u32 = millau_runtime::VERSION.spec_version;
				const TARGET_SPEC_VERSION: u32 = rialto_runtime::VERSION.spec_version;

				$generic
			}
		}
	};
}

impl SwapTokens {
	/// Run the command.
	pub async fn run(self) -> anyhow::Result<()> {
		select_bridge!(self.bridge, {
			let source_client = self.source.to_client::<Source>().await?;
			let source_sign = self.source_sign.to_keypair::<Target>()?;
			let target_client = self.target.to_client::<Target>().await?;
			let target_sign = self.target_sign.to_keypair::<Target>()?;

			// names of variables in this function are matching names used by the `pallet-bridge-token-swap`

			// accounts that are directly controlled by participants
			let source_account_at_this_chain: AccountIdOf<Source> = source_sign.public().into();
			let target_account_at_bridged_chain: AccountIdOf<Target> = target_sign.public().into();

			// balances that we're going to swap
			let source_balance_at_this_chain: BalanceOf<Source> = self.source_balance.cast().into();
			let target_balance_at_bridged_chain: BalanceOf<Target> = self.target_balance.cast().into();

			// prepare token swap intention
			let (can_claim_at_block_number, token_swap_type) = prepare_token_swap_type(
				&source_client,
				self.swap_type,
			).await?;
			let token_swap = bp_token_swap::TokenSwap {
				swap_type: token_swap_type,
				source_balance_at_this_chain,
				source_account_at_this_chain: source_account_at_this_chain.clone(),
				target_balance_at_bridged_chain,
				target_account_at_bridged_chain: target_account_at_bridged_chain.clone(),
			};

			// group all accounts that will be used later
			let accounts = TokenSwapAccounts {
				source_account_at_bridged_chain: derive_target_account_from_source_account(
					bp_runtime::SourceAccount::Account(source_account_at_this_chain.clone())
				),
				target_account_at_this_chain: derive_source_account_from_target_account(
					bp_runtime::SourceAccount::Account(target_account_at_bridged_chain.clone())
				),
				source_account_at_this_chain,
				target_account_at_bridged_chain,
				swap_account: FromSwapToThisAccountIdConverter::convert(token_swap.using_encoded(blake2_256).into()),
			};

			// account balances are used to demonstrate what's happening :)
			let initial_balances = read_account_balances(&accounts, &source_client, &target_client).await?;

			// prepare `Currency::transfer` call that will happen at the target chain
			let bridged_currency_transfer: CallOf<Target> = pallet_balances::Call::transfer(
				accounts.source_account_at_bridged_chain.clone(),
				target_balance_at_bridged_chain,
			).into();
			let bridged_currency_transfer_weight = bridged_currency_transfer.get_dispatch_info().weight;

			// sign message
			let bridged_chain_spec_version = TARGET_SPEC_VERSION;
			let signature_payload = pallet_bridge_dispatch::account_ownership_digest(
				&bridged_currency_transfer,
				&accounts.swap_account,
				&bridged_chain_spec_version,
				SOURCE_CHAIN_ID,
				TARGET_CHAIN_ID,
			);

				/*let digest = account_ownership_digest(
					&call,
					source_account_id,
					message.spec_version,
					source_chain,
					target_chain,
				);*/

			let bridged_currency_transfer_signature = target_sign.sign(&signature_payload).into();

			// prepare `create_swap` call
			let target_public_at_bridged_chain: AccountPublicOf<Target> = target_sign.public().into();
			let swap_delivery_and_dispatch_fee: BalanceOf<Source> = 1_000_000_000_000; // TODO: remove or compute
			let create_swap_call: CallOf<Source> = pallet_bridge_token_swap::Call::create_swap(
				token_swap.clone(),
				target_public_at_bridged_chain,
				swap_delivery_and_dispatch_fee,
				bridged_chain_spec_version,
				bridged_currency_transfer.encode(),
				bridged_currency_transfer_weight,
				bridged_currency_transfer_signature,
			).into();

			// before calling something that may fail, log what we're trying to do
			log::info!(target: "bridge", "Starting swap: {:?}", token_swap);
			log::info!(target: "bridge", "Swap accounts: {:?}", accounts);
			log::info!(target: "bridge", "Initial account balances: {:?}", initial_balances);

			// start tokens swap
			let source_genesis_hash = *source_client.genesis_hash();
let xxx = source_sign.clone();
			let swap_created_at = wait_until_transaction_is_finalized::<Source>(
				source_client.submit_and_watch_signed_extrinsic(
					accounts.source_account_at_this_chain.clone(),
					move |_, transaction_nonce| Bytes(Source::sign_transaction(
						source_genesis_hash,
						&xxx,
						relay_substrate_client::TransactionEra::immortal(),
						UnsignedTransaction::new(create_swap_call, transaction_nonce),
					).encode())
				).await?,
			).await?;

			// read state of swap after it has been created
			let token_swap_hash: H256 = token_swap.using_encoded(blake2_256).into();
			let token_swap_storage_key = bp_runtime::storage_map_final_key_identity(
				TOKEN_SWAP_PALLET_NAME,
				pallet_bridge_token_swap::PENDING_SWAPS_MAP_NAME,
				token_swap_hash.as_ref(),
			);
			match read_token_swap_state(&source_client, swap_created_at, &token_swap_storage_key).await? {
				Some(bp_token_swap::TokenSwapState::Started) => {
					log::info!(target: "bridge", "Swap has been successfully started");
					let intermediate_balances = read_account_balances(&accounts, &source_client, &target_client).await?;
					log::info!(target: "bridge", "Intermediate balances: {:?}", intermediate_balances);
				},
				Some(token_swap_state) => return Err(anyhow::format_err!(
					"Fresh token swap has unexpected state: {:?}",
					token_swap_state,
				)),
				None => return Err(anyhow::format_err!("Failed to start token swap")),
			};

			// wait until message is dispatched at the target chain and dispatch result delivered back to source chain
			let token_swap_state = wait_until_token_swap_state_is_changed(
				&source_client,
				&token_swap_storage_key,
				bp_token_swap::TokenSwapState::Started,
			).await?;
			let is_transfer_succeeded = match token_swap_state {
				Some(bp_token_swap::TokenSwapState::Started) => unreachable!(
					"wait_until_token_swap_state_is_changed only returns if state is not Started; qed",
				),
				None => return Err(anyhow::format_err!("Fresh token swap has disappeared unexpectedly")),
				Some(bp_token_swap::TokenSwapState::Confirmed) => {
					log::info!(
						target: "bridge",
						"Transfer has been successfully dispatched at the target chain. Swap can be claimed",
					);
					true
				},
				Some(bp_token_swap::TokenSwapState::Failed) => {
					log::info!(
						target: "bridge",
						"Transfer has been dispatched with an error at the target chain. Swap can be cancelled",
					);
					false
				}
			};

			let intermediate_balances = read_account_balances(&accounts, &source_client, &target_client).await?;
			log::info!(target: "bridge", "Intermediate balances: {:?}", intermediate_balances);

			// transfer has been dispatched, but we may need to wait until block where swap can be claimed/cancelled
			wait_until_block_number(&source_client, can_claim_at_block_number).await?;

			// finally we may claim or cancel the swap
			if is_transfer_succeeded {
				log::info!(target: "bridge", "Claiming the swap swap");

				// send `claim_swap` call over the bridge
				let lane_id = Default::default(); // TODO
				let swap_delivery_and_dispatch_fee: BalanceOf<Target> = 1_000_000_000_000; // TODO: remove or compute
				let claim_swap_call: CallOf<Source> = pallet_bridge_token_swap::Call::claim_swap(token_swap).into();
				let send_message_call: CallOf<Target> = pallet_bridge_messages::Call::send_message(
					lane_id,
					bp_message_dispatch::MessagePayload {
						spec_version: SOURCE_SPEC_VERSION,
						weight: claim_swap_call.get_dispatch_info().weight,
						origin: bp_message_dispatch::CallOrigin::SourceAccount(accounts.target_account_at_bridged_chain.clone()),
						dispatch_fee_payment: bp_runtime::messages::DispatchFeePayment::AtSourceChain,
						call: claim_swap_call.encode(),
					},
					swap_delivery_and_dispatch_fee,
				).into();
				let target_genesis_hash = *target_client.genesis_hash();
				let _ = wait_until_transaction_is_finalized::<Target>(
					target_client.submit_and_watch_signed_extrinsic(
						accounts.target_account_at_bridged_chain.clone(),
						move |_, transaction_nonce| Bytes(Target::sign_transaction(
							target_genesis_hash,
							&target_sign,
							relay_substrate_client::TransactionEra::immortal(),
							UnsignedTransaction::new(send_message_call, transaction_nonce),
						).encode())
					).await?,
				).await?;

				// wait until swap state is updated
				let token_swap_state = wait_until_token_swap_state_is_changed(
					&source_client,
					&token_swap_storage_key,
					bp_token_swap::TokenSwapState::Confirmed,
				).await?;
				if token_swap_state != None {
					return Err(anyhow::format_err!("Confirmed token swap state has been changed to {:?} unexpectedly"));
				}

				let final_balances = read_account_balances(&accounts, &source_client, &target_client).await?;
				log::info!(target: "bridge", "Final account balances: {:?}", final_balances);
			} else {
				log::info!(target: "bridge", "Cancelling the swap");
				let cancel_swap_call: CallOf<Source> = pallet_bridge_token_swap::Call::cancel_swap(token_swap.clone()).into();
				let _ = wait_until_transaction_is_finalized::<Source>(
					source_client.submit_and_watch_signed_extrinsic(
						accounts.source_account_at_this_chain.clone(),
						move |_, transaction_nonce| Bytes(Source::sign_transaction(
							source_genesis_hash,
							&source_sign,
							relay_substrate_client::TransactionEra::immortal(),
							UnsignedTransaction::new(cancel_swap_call, transaction_nonce),
						).encode())
					).await?,
				).await?;

				let final_balances = read_account_balances(&accounts, &source_client, &target_client).await?;
				log::info!(target: "bridge", "Final account balances: {:?}", final_balances);
			};
/*
			loop {
				async_std::task::sleep(sleep_interval).await;

				let balances

				let source_account_at_this_chain_balance: BalanceOf<Source> = source_client
					.free_native_balance(source_account_at_this_chain.clone())
					.await?;
				let source_account_at_bridged_chain_balance: BalanceOf<Target> = target_client
					.free_native_balance(source_account_at_bridged_chain.clone())
					.await?;
				let target_account_at_bridged_chain_balance: BalanceOf<Target> = target_client
					.free_native_balance(target_account_at_bridged_chain.clone())
					.await?;
				let target_account_at_this_chain_balance: BalanceOf<Source> = source_client
					.free_native_balance(target_account_at_this_chain.clone())
					.await?;
				log::info!(
					target: "bridge",
					"SourceAccountAtThisChain: {:?} Balance: {:?}\n\t
					SourceAccountAtBridgedChain: {:?} Balance: {:?}\n\t
					TargetAccountAtBridgedChain: {:?} Balance: {:?}\n\t
					TargetAccountAtThisChain: {:?} Balance: {:?}"
					source_account_at_this_chain,
					source_account_at_this_chain_balance,
					source_account_at_bridged_chain,
					source_account_at_bridged_chain_balance,
					target_account_at_bridged_chain,
					target_account_at_bridged_chain_balance,
					target_account_at_this_chain,
					target_account_at_this_chain_balance,
				);

				match source_account_at_bridged_chain_balance.checked_sub(initial_source_account_at_bridged_chain_balance) {
					Some(difference) if difference == 0 => {},
					Some(difference) if difference == target_balance_at_bridged_chain => {
						log::info!(
							target: "bridge",
							"Swap has occured at target chain. Waiting",
							source_account_at_bridged_chain,
							source_account_at_bridged_chain_balance,
							initial_source_account_at_bridged_chain_balance,
						);
					},
					None => 
				}
				if  !=  {
					 
				}
			}*/

			Ok(())
		})
	}
}

/// Accounts that are participating in the swap.
#[derive(Debug)]
struct TokenSwapAccounts<ThisAccountId, BridgedAccountId> {
	source_account_at_this_chain: ThisAccountId,
	source_account_at_bridged_chain: BridgedAccountId,
	target_account_at_bridged_chain: BridgedAccountId,
	target_account_at_this_chain: ThisAccountId,
	swap_account: ThisAccountId,
}

/// Swap accounts balances.
#[derive(Debug)]
struct TokenSwapBalances<ThisBalance, BridgedBalance> {
	source_account_at_this_chain_balance: Option<ThisBalance>,
	source_account_at_bridged_chain_balance: Option<BridgedBalance>,
	target_account_at_bridged_chain_balance: Option<BridgedBalance>,
	target_account_at_this_chain_balance: Option<ThisBalance>,
	swap_account_balance: Option<ThisBalance>,
}

/// Prepare token swap type.
///
/// Apart from the token swap type, it also returns number of block number at which claim
/// can be claimed or cancelled.
async fn prepare_token_swap_type<Source: Chain>(
	source_client: &Client<Source>,
	token_swap_type: TokenSwapType,
) -> anyhow::Result<(BlockNumberOf<Source>, bp_token_swap::TokenSwapType<BlockNumberOf<Source>>)> {
	match token_swap_type {
		TokenSwapType::TemporaryTargetAccountAtBridgedChain =>
			Ok((Zero::zero(), bp_token_swap::TokenSwapType::TemporaryTargetAccountAtBridgedChain)),
		TokenSwapType::LockClaimUntilBlock { blocks_before_expire, swap_nonce } => {
			let blocks_before_expire: BlockNumberOf<Source> = blocks_before_expire.into();
			let current_source_block_number = *source_client.best_header().await?.number();
			let last_available_block_number = current_source_block_number + blocks_before_expire;
			let swap_nonce = swap_nonce.unwrap_or_else(|| {
				U256::from(random::<u128>()).overflowing_mul(U256::from(random::<u128>())).0
			});
			Ok((
				last_available_block_number + One::one(),
				bp_token_swap::TokenSwapType::LockClaimUntilBlock(
					last_available_block_number,
					swap_nonce,
				),
			))
		},
	}
}

/// Read swap accounts balances.
async fn read_account_balances<Source: ChainWithBalances, Target: ChainWithBalances>(
	accounts: &TokenSwapAccounts<AccountIdOf<Source>, AccountIdOf<Target>>,
	source_client: &Client<Source>,
	target_client: &Client<Target>,
) -> anyhow::Result<TokenSwapBalances<BalanceOf<Source>, BalanceOf<Target>>> {
	Ok(TokenSwapBalances {
		source_account_at_this_chain_balance: read_account_balance(
			&source_client,
			&accounts.source_account_at_this_chain,
		).await?,
		source_account_at_bridged_chain_balance: read_account_balance(
			&target_client,
			&accounts.source_account_at_bridged_chain,
		).await?,
		target_account_at_bridged_chain_balance: read_account_balance(
			&target_client,
			&accounts.target_account_at_bridged_chain,
		).await?,
		target_account_at_this_chain_balance: read_account_balance(
			&source_client,
			&accounts.target_account_at_this_chain,
		).await?,
		swap_account_balance: read_account_balance(&source_client, &accounts.swap_account).await?,
	})
}

/// Read account balance.
async fn read_account_balance<C: ChainWithBalances>(
	client: &Client<C>,
	account: &AccountIdOf<C>
) -> anyhow::Result<Option<BalanceOf<C>>> {
	match client.free_native_balance(account.clone()).await {
		Ok(balance) => Ok(Some(balance)),
		Err(SubstrateError::AccountDoesNotExist) => Ok(None),
		Err(error) => Err(anyhow::format_err!(
			"Failed to read balance of {} account {:?}: {:?}",
			C::NAME,
			account,
			error,
		)),
	}
}

/// Wait until transaction is included into finalized block.
///
/// Returns the hash of the finalized block with transaction.
async fn wait_until_transaction_is_finalized<C: Chain>(
	subscription: Subscription<TransactionStatusOf<C>>,
) -> anyhow::Result<HashOf<C>> {
	loop {
		let transaction_status = subscription.next().await?;
		match transaction_status {
			Some(TransactionStatusOf::<C>::FinalityTimeout(_))
				| Some(TransactionStatusOf::<C>::Usurped(_))
				| Some(TransactionStatusOf::<C>::Dropped)
				| Some(TransactionStatusOf::<C>::Invalid)
				| None => return Err(anyhow::format_err!(
					"We've been waiting for finalization of {} transaction, but it now has the {:?} status",
					C::NAME,
					transaction_status,
				)),
			Some(TransactionStatusOf::<C>::Finalized(block_hash)) => {
				log::trace!(
					target: "bridge",
					"{} transaction has been finalized at block {}",
					C::NAME,
					block_hash,
				);
				return Ok(block_hash);
			}
			_ => {
				log::trace!(
					target: "bridge",
					"Received intermediate status of {} transaction: {:?}",
					C::NAME,
					transaction_status,
				);
			}
		}
	}
}

/// Waits until token swap state is changed from `Started` to something else.
async fn wait_until_token_swap_state_is_changed<C: Chain>(
	client: &Client<C>,
	swap_state_storage_key: &StorageKey,
	previous_token_swap_state: bp_token_swap::TokenSwapState,
) -> anyhow::Result<Option<bp_token_swap::TokenSwapState>> {
	log::trace!(target: "bridge", "Waiting for token swap state change");
	loop {
		async_std::task::sleep(C::AVERAGE_BLOCK_INTERVAL).await;

		let best_block = client.best_finalized_header_number().await?;
		let best_block_hash = client.block_hash_by_number(best_block).await?;
		log::trace!(target: "bridge", "Inspecting {} block {}/{}", C::NAME, best_block, best_block_hash);

		let token_swap_state = read_token_swap_state(client, best_block_hash, swap_state_storage_key).await?;
		match token_swap_state {
			Some(new_token_swap_state) if new_token_swap_state == previous_token_swap_state => {},
			_ => {
				log::trace!(
					target: "bridge",
					"Token swap state has been changed from {:?} to {:?}",
					previous_token_swap_state,
					token_swap_state,
				);
				return Ok(token_swap_state)
			},
		}
	}
}

/// Waits until block with given number is finalized.
async fn wait_until_block_number<C: Chain>(
	client: &Client<C>,
	required_block_number: BlockNumberOf<C>,
) -> anyhow::Result<()> {
	log::trace!(target: "bridge", "Waiting for token swap state change");
	loop {
		async_std::task::sleep(C::AVERAGE_BLOCK_INTERVAL).await;

		let best_block = client.best_finalized_header_number().await?;
		let best_block_hash = client.block_hash_by_number(best_block).await?;
		log::trace!(target: "bridge", "Inspecting {} block {}/{}", C::NAME, best_block, best_block_hash);

		if best_block >= required_block_number {
			return Ok(());
		}
	}
}

/// Read state of the active token swap.
async fn read_token_swap_state<C: Chain>(
	client: &Client<C>,
	at_block: C::Hash,
	swap_state_storage_key: &StorageKey,
) -> anyhow::Result<Option<bp_token_swap::TokenSwapState>> {
	Ok(client.storage_value(swap_state_storage_key.clone(), Some(at_block)).await?)
}
