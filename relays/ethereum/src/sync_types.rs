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

/// Ethereum header Id.
#[derive(Debug, Clone, Copy, Eq, Hash, PartialEq)]
pub struct HeaderId<Hash, Number>(pub Number, pub Hash);

/// Ethereum header synchronization status.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HeaderStatus {
	/// Header is unknown.
	Unknown,
	/// Header is in MaybeOrphan queue.
	MaybeOrphan,
	/// Header is in Orphan queue.
	Orphan,
	/// Header is in MaybeExtra queue.
	MaybeExtra,
	/// Header is in Extra queue.
	Extra,
	/// Header is in Ready queue.
	Ready,
	/// Header is in Incomplete queue.
	Incomplete,
	/// Header has been recently submitted to the target node.
	Submitted,
	/// Header is known to the target node.
	Synced,
}

/// Error type that can signal connection errors.
pub trait MaybeConnectionError {
	/// Returns true if error (maybe) represents connection error.
	fn is_connection_error(&self) -> bool;
}

/// Headers synchronization pipeline.
pub trait HeadersSyncPipeline: Clone + Copy {
	/// Name of the headers source.
	const SOURCE_NAME: &'static str;
	/// Name of the headers target.
	const TARGET_NAME: &'static str;

	/// Headers we're syncing are identified by this hash.
	type Hash: Eq + Clone + Copy + std::fmt::Debug + std::fmt::Display + std::hash::Hash;
	/// Headers we're syncing are identified by this number.
	type Number: From<u32>
		+ Ord
		+ Clone
		+ Copy
		+ std::fmt::Debug
		+ std::fmt::Display
		+ std::hash::Hash
		+ std::ops::Add<Output = Self::Number>
		+ std::ops::Sub<Output = Self::Number>
		+ num_traits::Saturating
		+ num_traits::Zero
		+ num_traits::One;
	/// Type of header that we're syncing.
	type Header: Clone + std::fmt::Debug + SourceHeader<Self::Hash, Self::Number>;
	/// Type of extra data for the header that we're receiving from the source node.
	type Extra: Clone + std::fmt::Debug;
	/// Type of data required to 'complete' header that we're receiving from the source node.
	type Completion: Clone + std::fmt::Debug;

	/// Function used to estimate size of target-encoded header.
	fn estimate_size(source: &QueuedHeader<Self>) -> usize;
}

/// Header that we're receiving from source node.
pub trait SourceHeader<Hash, Number> {
	/// Returns ID of header.
	fn id(&self) -> HeaderId<Hash, Number>;
	/// Returns ID of parent header.
	fn parent_id(&self) -> HeaderId<Hash, Number>;
}

/// Header how it's stored in the synchronization queue.
#[derive(Clone, Debug, Default)]
#[cfg_attr(test, derive(PartialEq))]
pub struct QueuedHeader<P: HeadersSyncPipeline> {
	header: P::Header,
	extra: Option<P::Extra>,
}

impl<P: HeadersSyncPipeline> QueuedHeader<P> {
	/// Creates new queued header.
	pub fn new(header: P::Header) -> Self {
		QueuedHeader { header, extra: None }
	}

	/// Returns ID of header.
	pub fn id(&self) -> HeaderId<P::Hash, P::Number> {
		self.header.id()
	}

	/// Returns ID of parent header.
	pub fn parent_id(&self) -> HeaderId<P::Hash, P::Number> {
		self.header.parent_id()
	}

	/// Returns reference to header.
	pub fn header(&self) -> &P::Header {
		&self.header
	}

	/// Returns reference to associated extra data.
	pub fn extra(&self) -> &Option<P::Extra> {
		&self.extra
	}

	/// Extract header and extra from self.
	pub fn extract(self) -> (P::Header, Option<P::Extra>) {
		(self.header, self.extra)
	}

	/// Set associated extra data.
	pub fn set_extra(mut self, extra: P::Extra) -> Self {
		self.extra = Some(extra);
		self
	}
}
