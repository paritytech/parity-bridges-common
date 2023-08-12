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

use async_trait::async_trait;
use finality_relay::{FinalityPipeline, SourceClientBase};
use relay_utils::{relay_loop::Client as RelayClient, TransactionTracker};

pub trait EquivocationDetectionPipeline: FinalityPipeline {
	/// The type of the equivocation proof.
	type EquivocationProof;
}

/// Source client used in equivocation detection loop.
#[async_trait]
pub trait SourceClient<P: EquivocationDetectionPipeline>: SourceClientBase<P> {
	/// Transaction tracker to track submitted transactions.
	type TransactionTracker: TransactionTracker;

	/// Report equivocation.
	async fn report_equivocation(
		&self,
		at: P::Hash,
		equivocation: P::EquivocationProof,
	) -> Result<Self::TransactionTracker, Self::Error>;
}

/// Target client used in equivocation detection loop.
#[async_trait]
pub trait TargetClient<P: EquivocationDetectionPipeline>: RelayClient {}
