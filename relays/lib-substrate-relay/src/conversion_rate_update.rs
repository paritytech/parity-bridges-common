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

//! Tools for updating conversion rate that is stored in the runtime storage.

use crate::{messages_lane::SubstrateMessageLane, TransactionParams};

use codec::Encode;
use relay_substrate_client::{AccountIdOf, AccountKeyPairOf, CallOf, Chain, Client, TransactionEra, TransactionSignScheme, UnsignedTransaction};
use relay_utils::metrics::F64SharedRef;
use sp_core::{Bytes, Pair};
use std::time::Duration;

/// Duration between updater iterations.
const SLEEP_DURATION: Duration = Duration::from_secs(60);

/// Update-conversion-rate transaction status.
#[derive(Debug, Clone, Copy, PartialEq)]
enum TransactionStatus {
	/// We have not submitted any transaction recently.
	Idle,
	/// We have recently submitted transaction that should update conversion rate.
	Submitted(f64),
}

/// Different ways of building 'update conversion rate' calls.
pub trait UpdateConversionRateCallBuilder<C: Chain> {
	/// Given conversion rate, build call that updates conversion rate in given chain runtime storage.
	fn build_update_conversion_rate_call(conversion_rate: f64) -> anyhow::Result<CallOf<C>>;
}

impl<C: Chain> UpdateConversionRateCallBuilder<C> for () {
	fn build_update_conversion_rate_call(_conversion_rate: f64) -> anyhow::Result<CallOf<C>> {
		Err(anyhow::format_err!("Conversion rate update is not supported at {}", C::NAME))
	}
}

/// Macro that generates `UpdateConversionRateCallBuilder` implementation for the case when
/// you have a direct access to the source chain runtime.
#[rustfmt::skip]
#[macro_export]
macro_rules! generate_direct_update_conversion_rate_call_builder {
	(
		$source_chain:ident,
		$mocked_builder:ident,
		$runtime:ty,
		$instance:ty,
		$parameter:path
	) => {
		pub struct $mocked_builder;

		impl $crate::conversion_rate_update::UpdateConversionRateCallBuilder<$source_chain>
			for $mocked_builder
		{
			fn build_update_conversion_rate_call(
				conversion_rate: f64,
			) -> anyhow::Result<relay_substrate_client::CallOf<$source_chain>> {
				Ok(pallet_bridge_messages::Call::update_pallet_parameter::<$runtime, $instance> {
					parameter: $parameter(sp_runtime::FixedU128::from_float(conversion_rate)),
				}.into())
			}
		}
	};
}

/// Macro that generates `UpdateConversionRateCallBuilder` implementation for the case when
/// you only have an access to the mocked version of source chain runtime. In this case you
/// should provide "name" of the call variant for the bridge messages calls, the "name" of
/// the variant for the `update_pallet_parameter` call within that first option and the name
/// of the conversion rate parameter itself.
#[rustfmt::skip]
#[macro_export]
macro_rules! generate_mocked_update_conversion_rate_call_builder {
	(
		$source_chain:ident,
		$mocked_builder:ident,
		$bridge_messages:path,
		$update_pallet_parameter:path,
		$parameter:path
	) => {
		pub struct $mocked_builder;

		impl $crate::conversion_rate_update::UpdateConversionRateCallBuilder<$source_chain>
			for $mocked_builder
		{
			fn build_update_conversion_rate_call(
				conversion_rate: f64,
			) -> anyhow::Result<relay_substrate_client::CallOf<$source_chain>> {
				Ok($bridge_messages($update_pallet_parameter($parameter(
					sp_runtime::FixedU128::from_float(conversion_rate),
				))))
			}
		}
	};
}

/// Run infinite conversion rate updater loop.
///
/// The loop is maintaining the Left -> Right conversion rate, used as `RightTokens = LeftTokens *
/// Rate`.
pub fn run_conversion_rate_update_loop<Lane, Sign>(
	client: Client<Lane::SourceChain>,
	transaction_params: TransactionParams<AccountKeyPairOf<Sign>>,
	left_to_right_stored_conversion_rate: F64SharedRef,
	left_to_base_conversion_rate: F64SharedRef,
	right_to_base_conversion_rate: F64SharedRef,
	max_difference_ratio: f64,
) where
	Lane: SubstrateMessageLane,
	Sign: TransactionSignScheme<Chain = Lane::SourceChain>,
	AccountIdOf<Lane::SourceChain>: From<<AccountKeyPairOf<Sign> as Pair>::Public>,
{
	async_std::task::spawn(async move {
		let mut transaction_status = TransactionStatus::Idle;
		loop {
			async_std::task::sleep(SLEEP_DURATION).await;
			let maybe_new_conversion_rate = maybe_select_new_conversion_rate(
				&mut transaction_status,
				&left_to_right_stored_conversion_rate,
				&left_to_base_conversion_rate,
				&right_to_base_conversion_rate,
				max_difference_ratio,
			)
			.await;
			if let Some((prev_conversion_rate, new_conversion_rate)) = maybe_new_conversion_rate {
				log::info!(
					target: "bridge",
					"Going to update {} -> {} (on {}) conversion rate to {}.",
					Lane::TargetChain::NAME,
					Lane::SourceChain::NAME,
					Lane::SourceChain::NAME,
					new_conversion_rate,
				);

				let result = update_target_to_source_conversion_rate::<Lane, Sign>(
					client.clone(),
					transaction_params.clone(),
					new_conversion_rate,
				).await;
				match result {
					Ok(()) => {
						transaction_status = TransactionStatus::Submitted(prev_conversion_rate);
					},
					Err(error) => {
						log::trace!(
							target: "bridge",
							"Failed to submit conversion rate update transaction: {:?}",
							error,
						);
					},
				}
			}
		}
	});
}

/// Select new conversion rate to submit to the node.
async fn maybe_select_new_conversion_rate(
	transaction_status: &mut TransactionStatus,
	left_to_right_stored_conversion_rate: &F64SharedRef,
	left_to_base_conversion_rate: &F64SharedRef,
	right_to_base_conversion_rate: &F64SharedRef,
	max_difference_ratio: f64,
) -> Option<(f64, f64)> {
	let left_to_right_stored_conversion_rate =
		(*left_to_right_stored_conversion_rate.read().await)?;
	match *transaction_status {
		TransactionStatus::Idle => (),
		TransactionStatus::Submitted(previous_left_to_right_stored_conversion_rate) => {
			// we can't compare float values from different sources directly, so we only care
			// whether the stored rate has been changed or not. If it has been changed, then we
			// assume that our proposal has been accepted.
			//
			// float comparison is ok here, because we compare same-origin (stored in runtime
			// storage) values and if they are different, it means that the value has actually been
			// updated
			#[allow(clippy::float_cmp)]
			if previous_left_to_right_stored_conversion_rate == left_to_right_stored_conversion_rate
			{
				// the rate has not been changed => we won't submit any transactions until it is
				// accepted, or the rate is changed by someone else
				return None
			}

			*transaction_status = TransactionStatus::Idle;
		},
	}

	let left_to_base_conversion_rate = (*left_to_base_conversion_rate.read().await)?;
	let right_to_base_conversion_rate = (*right_to_base_conversion_rate.read().await)?;
	let actual_left_to_right_conversion_rate =
		right_to_base_conversion_rate / left_to_base_conversion_rate;

	let rate_difference =
		(actual_left_to_right_conversion_rate - left_to_right_stored_conversion_rate).abs();
	let rate_difference_ratio = rate_difference / left_to_right_stored_conversion_rate;
	if rate_difference_ratio < max_difference_ratio {
		return None
	}

	Some((left_to_right_stored_conversion_rate, actual_left_to_right_conversion_rate))
}

/// Update Target -> Source tokens conversion rate, stored in the Source runtime storage.
pub async fn update_target_to_source_conversion_rate<Lane, Sign>(
	client: Client<Lane::SourceChain>,
	transaction_params: TransactionParams<AccountKeyPairOf<Sign>>,
	updated_rate: f64,
) -> anyhow::Result<()> where
	Lane: SubstrateMessageLane,
	Sign: TransactionSignScheme<Chain = Lane::SourceChain>,
	AccountIdOf<Lane::SourceChain>: From<<AccountKeyPairOf<Sign> as Pair>::Public>,
{
	let genesis_hash = *client.genesis_hash();
	let signer_id = transaction_params.signer.public().into();
	let call = Lane::TargetToSourceChainConversionRateUpdateBuilder::build_update_conversion_rate_call(
		updated_rate
	)?;
	client
		.submit_signed_extrinsic(signer_id, move |best_block_id, transaction_nonce| {
			Bytes(
				Sign::sign_transaction(
					genesis_hash,
					&transaction_params.signer,
					TransactionEra::new(best_block_id, transaction_params.mortality),
					UnsignedTransaction::new(call, transaction_nonce),
				)
					.encode(),
			)
		})
		.await
		.map(drop)
		.map_err(|err| anyhow::format_err!("{:?}", err))
}

#[cfg(test)]
mod tests {
	use super::*;
	use async_std::sync::{Arc, RwLock};

	fn test_maybe_select_new_conversion_rate(
		mut transaction_status: TransactionStatus,
		stored_conversion_rate: Option<f64>,
		left_to_base_conversion_rate: Option<f64>,
		right_to_base_conversion_rate: Option<f64>,
		max_difference_ratio: f64,
	) -> (Option<(f64, f64)>, TransactionStatus) {
		let stored_conversion_rate = Arc::new(RwLock::new(stored_conversion_rate));
		let left_to_base_conversion_rate = Arc::new(RwLock::new(left_to_base_conversion_rate));
		let right_to_base_conversion_rate = Arc::new(RwLock::new(right_to_base_conversion_rate));
		let result = async_std::task::block_on(maybe_select_new_conversion_rate(
			&mut transaction_status,
			&stored_conversion_rate,
			&left_to_base_conversion_rate,
			&right_to_base_conversion_rate,
			max_difference_ratio,
		));
		(result, transaction_status)
	}

	#[test]
	fn rate_is_not_updated_when_transaction_is_submitted() {
		assert_eq!(
			test_maybe_select_new_conversion_rate(
				TransactionStatus::Submitted(10.0),
				Some(10.0),
				Some(1.0),
				Some(1.0),
				0.0
			),
			(None, TransactionStatus::Submitted(10.0)),
		);
	}

	#[test]
	fn transaction_state_is_changed_to_idle_when_stored_rate_shanges() {
		assert_eq!(
			test_maybe_select_new_conversion_rate(
				TransactionStatus::Submitted(1.0),
				Some(10.0),
				Some(1.0),
				Some(1.0),
				100.0
			),
			(None, TransactionStatus::Idle),
		);
	}

	#[test]
	fn transaction_is_not_submitted_when_left_to_base_rate_is_unknown() {
		assert_eq!(
			test_maybe_select_new_conversion_rate(
				TransactionStatus::Idle,
				Some(10.0),
				None,
				Some(1.0),
				0.0
			),
			(None, TransactionStatus::Idle),
		);
	}

	#[test]
	fn transaction_is_not_submitted_when_right_to_base_rate_is_unknown() {
		assert_eq!(
			test_maybe_select_new_conversion_rate(
				TransactionStatus::Idle,
				Some(10.0),
				Some(1.0),
				None,
				0.0
			),
			(None, TransactionStatus::Idle),
		);
	}

	#[test]
	fn transaction_is_not_submitted_when_stored_rate_is_unknown() {
		assert_eq!(
			test_maybe_select_new_conversion_rate(
				TransactionStatus::Idle,
				None,
				Some(1.0),
				Some(1.0),
				0.0
			),
			(None, TransactionStatus::Idle),
		);
	}

	#[test]
	fn transaction_is_not_submitted_when_difference_is_below_threshold() {
		assert_eq!(
			test_maybe_select_new_conversion_rate(
				TransactionStatus::Idle,
				Some(1.0),
				Some(1.0),
				Some(1.01),
				0.02
			),
			(None, TransactionStatus::Idle),
		);
	}

	#[test]
	fn transaction_is_submitted_when_difference_is_above_threshold() {
		assert_eq!(
			test_maybe_select_new_conversion_rate(
				TransactionStatus::Idle,
				Some(1.0),
				Some(1.0),
				Some(1.03),
				0.02
			),
			(Some((1.0, 1.03)), TransactionStatus::Idle),
		);
	}
}
