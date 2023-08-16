// Copyright 2019-2023 Parity Technologies (UK) Ltd.
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

use crate::{
	reporter::EquivocationsReporter, EquivocationDetectionPipeline, HeaderFinalityInfo,
	SourceClient, TargetClient,
};

use bp_header_chain::{FinalityProof, FindEquivocations};
use finality_relay::{FinalityProofsBuf, FinalityProofsStream};
use futures::{select, FutureExt};
use num_traits::Saturating;
use relay_utils::{
	relay_loop::{reconnect_failed_client, RECONNECT_DELAY},
	FailedClient, MaybeConnectionError,
};
use std::{future::Future, time::Duration};

struct EquivocationReportingContext<P: EquivocationDetectionPipeline> {
	synced_header_hash: P::Hash,
	synced_verification_context: P::FinalityVerificationContext,
}

impl<P: EquivocationDetectionPipeline> EquivocationReportingContext<P> {
	async fn try_read_from_target<TC: TargetClient<P>>(
		target_client: &TC,
		at: P::TargetNumber,
	) -> Result<Option<Self>, TC::Error> {
		let maybe_best_synced_header_hash = target_client.best_synced_header_hash(at).await?;
		Ok(match maybe_best_synced_header_hash {
			Some(best_synced_header_hash) => Some(EquivocationReportingContext {
				synced_header_hash: best_synced_header_hash,
				synced_verification_context: target_client
					.finality_verification_context(at)
					.await?,
			}),
			None => None,
		})
	}

	fn update(&mut self, info: HeaderFinalityInfo<P>) {
		if let Some(new_verification_context) = info.new_verification_context {
			self.synced_header_hash = info.finality_proof.target_header_hash();
			self.synced_verification_context = new_verification_context;
		}
	}
}

/// Finality synchronization loop state.
struct EquivocationDetectionLoop<
	P: EquivocationDetectionPipeline,
	SC: SourceClient<P>,
	TC: TargetClient<P>,
> {
	source_client: SC,
	target_client: TC,

	from_block_num: P::TargetNumber,
	until_block_num: P::TargetNumber,

	reporter: EquivocationsReporter<P, SC>,

	finality_proofs_stream: FinalityProofsStream<P, SC>,
	finality_proofs_buf: FinalityProofsBuf<P>,
}

impl<P: EquivocationDetectionPipeline, SC: SourceClient<P>, TC: TargetClient<P>>
	EquivocationDetectionLoop<P, SC, TC>
{
	async fn reconnect_source_client(&mut self, e: SC::Error) -> bool {
		if e.is_connection_error() {
			reconnect_failed_client(
				FailedClient::Source,
				RECONNECT_DELAY,
				&mut self.source_client,
				&mut self.target_client,
			)
			.await;

			return true
		}

		false
	}

	async fn reconnect_target_client(&mut self, e: TC::Error) -> bool {
		if e.is_connection_error() {
			reconnect_failed_client(
				FailedClient::Target,
				RECONNECT_DELAY,
				&mut self.source_client,
				&mut self.target_client,
			)
			.await;

			return true
		}

		false
	}

	async fn update_until_block_num(&mut self) {
		self.until_block_num = match self.target_client.best_finalized_header_number().await {
			Ok(hdr_num) => hdr_num,
			Err(e) => {
				log::error!(
					target: "bridge",
					"Could not read best finalized header number from {}: {e:?}",
					P::TARGET_NAME,
				);

				// Reconnect target client and move on
				self.reconnect_target_client(e).await;
				return
			},
		};
	}

	async fn build_context(
		&mut self,
		block_num: P::TargetNumber,
	) -> Option<EquivocationReportingContext<P>> {
		match EquivocationReportingContext::try_read_from_target(
			&self.target_client,
			block_num.saturating_sub(1.into()),
		)
		.await
		{
			Ok(Some(context)) => Some(context),
			Ok(None) => None,
			Err(e) => {
				log::error!(
					target: "bridge",
					"Could not read {} `EquivocationReportingContext` from {} at block {block_num}: {e:?}",
					P::SOURCE_NAME,
					P::TARGET_NAME,
				);

				// Reconnect target client if needed and move on.
				self.reconnect_target_client(e).await;
				None
			},
		}
	}

	async fn synced_source_headers_at_target(
		&mut self,
		at: P::TargetNumber,
	) -> Vec<HeaderFinalityInfo<P>> {
		match self.target_client.synced_headers_finality_info(at).await {
			Ok(synced_headers) => synced_headers,
			Err(e) => {
				log::error!(
					target: "bridge",
					"Could not get {} headers synced to {} at block {at:?}",
					P::SOURCE_NAME,
					P::TARGET_NAME
				);

				// Reconnect in case of a connection error.
				self.reconnect_target_client(e).await;
				// And move on to the next block.
				vec![]
			},
		}
	}

	async fn report_equivocation(&mut self, at: P::Hash, equivocation: P::EquivocationProof) {
		match self.reporter.submit_report(&self.source_client, at, equivocation.clone()).await {
			Ok(_) => {},
			Err(e) => {
				log::error!(
					target: "bridge",
					"Could not submit equivocation report to {} for {equivocation:?}: {e:?}",
					P::SOURCE_NAME,
				);

				// Reconnect source client and move on
				self.reconnect_source_client(e).await;
			},
		}
	}

	async fn check_block(
		&mut self,
		block_num: P::TargetNumber,
		context: &mut EquivocationReportingContext<P>,
	) {
		let synced_headers = self.synced_source_headers_at_target(block_num).await;

		for synced_header in synced_headers {
			self.finality_proofs_buf.fill(&mut self.finality_proofs_stream);

			let equivocations = match P::EquivocationsFinder::find_equivocations(
				&context.synced_verification_context,
				&synced_header.finality_proof,
				self.finality_proofs_buf.buf().as_slice(),
			) {
				Ok(equivocations) => equivocations,
				Err(e) => {
					log::error!(
						target: "bridge",
						"Could not search for equivocations in the finality proof \
						for source header {:?} synced at target block {block_num:?}: {e:?}",
						synced_header.finality_proof.target_header_hash()
					);
					continue
				},
			};
			for equivocation in equivocations {
				self.report_equivocation(context.synced_header_hash, equivocation).await;
			}

			self.finality_proofs_buf
				.prune(synced_header.finality_proof.target_header_number(), None);
			context.update(synced_header);
		}
	}

	async fn run(&mut self, tick: Duration, exit_signal: impl Future<Output = ()>) {
		let exit_signal = exit_signal.fuse();
		futures::pin_mut!(exit_signal);

		loop {
			// Make sure that we are connected to the source finality proofs stream.
			match self.finality_proofs_stream.ensure_stream(&self.source_client).await {
				Ok(_) => {},
				Err(e) => {
					log::error!(
						target: "bridge",
						"Could not connect to the {} `FinalityProofsStream`: {e:?}",
						P::SOURCE_NAME,
					);

					// Reconnect to the source client if needed
					match self.reconnect_source_client(e).await {
						true => {
							// Connection error. Move on.
							continue
						},
						false => {
							// Irrecoverable error. End the loop.
							return
						},
					}
				},
			}
			// Check the status of the pending equivocation reports
			self.reporter.process_pending_reports().await;

			// Check the next block
			self.update_until_block_num().await;
			if self.from_block_num <= self.until_block_num {
				let mut context = match self.build_context(self.from_block_num).await {
					Some(context) => context,
					None => return,
				};

				self.check_block(self.from_block_num, &mut context).await;

				self.from_block_num = self.from_block_num.saturating_add(1.into());
			}

			select! {
				_ = async_std::task::sleep(tick).fuse() => {},
				_ = exit_signal => return,
			}
		}
	}

	pub async fn spawn(
		source_client: SC,
		target_client: TC,
		tick: Duration,
		exit_signal: impl Future<Output = ()>,
	) -> Result<(), FailedClient> {
		let mut equivocation_detection_loop = Self {
			source_client,
			target_client,
			from_block_num: 0.into(),
			until_block_num: 0.into(),
			reporter: EquivocationsReporter::<P, SC>::new(),
			finality_proofs_stream: FinalityProofsStream::new(),
			finality_proofs_buf: FinalityProofsBuf::new(vec![]),
		};

		equivocation_detection_loop.run(tick, exit_signal).await;
		Ok(())
	}
}

/// TODO: remove `#[allow(dead_code)]`
#[allow(dead_code)]
pub async fn run<P: EquivocationDetectionPipeline>(
	source_client: impl SourceClient<P>,
	target_client: impl TargetClient<P>,
	tick: Duration,
	exit_signal: impl Future<Output = ()> + 'static + Send,
) -> Result<(), relay_utils::Error> {
	let exit_signal = exit_signal.shared();
	relay_utils::relay_loop(source_client, target_client)
		.run(
			format!("{}_to_{}_EquivocationDetection", P::SOURCE_NAME, P::TARGET_NAME),
			move |source_client, target_client, _metrics| {
				EquivocationDetectionLoop::spawn(
					source_client,
					target_client,
					tick,
					exit_signal.clone(),
				)
			},
		)
		.await
}
