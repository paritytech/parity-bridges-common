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

//! One-way message lane types.

use crate::utils::HeaderId;

use std::fmt::Debug;

/// One-way message lane.
pub trait MessageLane {
	/// Name of the messages source.
	const SOURCE_NAME: &'static str;
	/// Name of the messages target.
	const TARGET_NAME: &'static str;

	/// Message nonce type.
	type MessageNonce: Clone + Copy + Debug + Default + From<u32> + Ord + std::ops::Add<Output = Self::MessageNonce>;

	/// Messages proof.
	type MessagesProof: Clone;

	/// Number of the source header.
	type SourceHeaderNumber: Clone + Debug + Default + Ord + PartialEq;
	/// Hash of the source header.
	type SourceHeaderHash: Clone + Debug + Default + PartialEq;

	/// Number of the target header.
	type TargetHeaderNumber: Clone + Debug + Default + Ord + PartialEq;
	/// Hash of the target header.
	type TargetHeaderHash: Clone + Debug + Default + PartialEq;
}

/// Source header id withing given one-way message lane.
pub type SourceHeaderIdOf<P> = HeaderId<<P as MessageLane>::SourceHeaderHash, <P as MessageLane>::SourceHeaderNumber>;

/// Target header id withing given one-way message lane.
pub type TargetHeaderIdOf<P> = HeaderId<<P as MessageLane>::TargetHeaderHash, <P as MessageLane>::TargetHeaderNumber>;

/// Withing single one-way lane we have three 'races' where we try to:
///
/// 1) relay new messages from source to target node;
/// 2) relay proof-of-receiving from target to source node;
/// 3) relay proof-of-processing from target no source node.
///
/// Direction of these races isn't always source -> target. So to distinguish between
/// one-way lane' source and target, let's call them race begin and race end.
#[derive(Debug)]
pub struct Race<Id, Nonce, Proof> {
	/// The nonce at race begin.
	pub nonce_at_start: Nonce,
	/// `nonce_at_start` has been read at this block.
	pub nonce_at_start_block: Id,
	/// Best nonce at race end.
	pub nonce_at_end: Nonce,
	/// Prepared proof, if any.
	pub proof: Option<(Id, Nonce, Nonce, Proof)>,
	/// Latest nonce that we have submitted, if any.
	pub submitted: Option<Nonce>,
}

impl<Id: Default, Nonce: Default, Proof> Default for Race<Id, Nonce, Proof> {
	fn default() -> Self {
		Race {
			nonce_at_start: Default::default(),
			nonce_at_start_block: Default::default(),
			nonce_at_end: Default::default(),
			proof: None,
			submitted: None,
		}
	}
}
