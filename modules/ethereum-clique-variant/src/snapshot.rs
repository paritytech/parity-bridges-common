// Copyright 2015-2020 Parity Technologies (UK) Ltd.
// This file is part of OpenEthereum.

// OpenEthereum is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// OpenEthereum is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with OpenEthereum.  If not, see <http://www.gnu.org/licenses/>.

use std::{
	collections::{BTreeSet, HashMap, VecDeque},
	fmt,
	time::{Duration, SystemTime, UNIX_EPOCH},
};

use crate::{
	error::{Error, Mismatch},
	utils::*,
	ChainTime,
};
use bp_eth_clique::{Address, CliqueHeader, DIFF_INTURN, DIFF_NOTURN, NULL_AUTHOR, SIGNING_DELAY_NOTURN_MS};
use rand::Rng;

/// How many CliqueBlockState to cache in the memory.
pub const SNAP_CACHE_NUM: usize = 128;

lazy_static! {
	/// key: header hash
	/// value: creator address
	static ref SNAPSHOT_BY_HASH: RwLock<LruCache<H256, Snapshot>> = RwLock::new(LruCache::new(SNAP_CACHE_NUM));
}

/// Clique state for each block.
#[cfg(not(test))]
#[derive(Clone, Debug, Default)]
pub struct Snapshot<CT: ChainTime> {
	/// a list of all valid signer, sorted by ascending order.
	signers: BTreeSet<Address>,
	/// a deque of recent signer, new entry should be pushed front, apply() modifies this.
	recent_signers: VecDeque<Address>,
	/// inturn signing should wait until this time
	pub next_timestamp_inturn: Option<CT>,
	/// noturn signing should wait until this time
	pub next_timestamp_noturn: Option<CT>,
}

#[cfg(test)]
#[derive(Clone, Debug, Default)]
pub struct Snapshot<CT: ChainTime> {
	/// a list of all valid signer, sorted by ascending order.
	pub signers: BTreeSet<Address>,
	/// a deque of recent signer, new entry should be pushed front, apply() modifies this.
	pub recent_signers: VecDeque<Address>,
	/// inturn signing should wait until this time
	pub next_timestamp_inturn: Option<CT>,
	/// noturn signing should wait until this time
	pub next_timestamp_noturn: Option<CT>,
}

impl fmt::Display for Snapshot {
	fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
		let signers: Vec<String> = self.signers.iter().map(|s| format!("{}", s,)).collect();

		let recent_signers: Vec<String> = self.recent_signers.iter().map(|s| format!("{}", s)).collect();

		write!(
			f,
			"Snapshot {{ \n signers: {:?} \n recent_signers: {:?}}}",
			signers, recent_signers
		)
	}
}

impl Snapshot {
	/// Create new state with given information, this is used creating new state from Checkpoint block.
	pub fn new(signers: BTreeSet<Address>) -> Self {
		Snapshot {
			signers,
			..Default::default()
		}
	}

	fn snap_no_backfill(&self, hash: &H256) -> Option<Snapshot> {
		self.SNAPSHOT_BY_HASH.write().get_mut(hash).cloned()
	}

	/// Construct an new snapshot from given checkpoint header.
	fn recover_from(&self, header: &Header, clique_variant_config: &CliqueVariantConfiguration) -> Result<Self, Error> {
		// must be checkpoint block header
		debug_assert_eq!(header.number() % self.epoch_length, 0);

		self.signers = extract_signers(header)?;

		// TODO(niklasad1): refactor to perform this check in the `Snapshot` constructor instead
		self.calc_next_timestamp(header.timestamp(), clique_variant_config.period)?;

		Ok(Self)
	}

	/// Get `Snapshot` for given header, backfill from last checkpoint if needed.
	fn retrieve<S: Storage>(
		&self,
		storage: &S,
		header: &Header,
		clique_variant_config: &CliqueVariantConfiguration,
	) -> Result<Snapshot, Error> {
		let mut snapshot_by_hash = SNAPSHOT_BY_HASH.write();
		if let Some(snap) = snapshot_by_hash.get_mut(&header.hash()) {
			return Ok(snap.clone());
		}
		// If we are looking for an checkpoint block state, we can directly reconstruct it.
		if header.number % self.epoch_length == 0 {
			let snap = self.recover_from(header)?;
			snapshot_by_hash.insert(header.hash(), snap.clone());
			return Ok(snap);
		}
		// BlockState is not found in memory, which means we need to reconstruct state from last checkpoint.
		let last_checkpoint_number = header.number() - header.number() % clique_variant_config.epoch_length as u64;
		debug_assert_ne!(last_checkpoint_number, header.number());

		// Catching up state, note that we don't really store block state for intermediary blocks,
		// for speed.
		let backfill_start = time::Instant::now();
		log::trace!(target: "snapshot",
					"Back-filling snapshot. last_checkpoint_number: {}, target: {}({}).",
					last_checkpoint_number, header.number(), header.hash());

		let chain: &mut VecDeque<CliqueHeader> =
			&mut VecDeque::with_capacity((header.number() - last_checkpoint_number + 1) as usize);

		// Put ourselves in.
		chain.push_front(header.clone());

		// populate chain to last checkpoint
		loop {
			let (last_parent_hash, last_num) = {
				let l = chain.front().expect("chain has at least one element; qed");
				(*l.parent_hash, l.number)
			};

			if last_num == last_checkpoint_number + 1 {
				break;
			}
			match Storage::header(last_parent_hash) {
				None => {
					return Err(Error::UnknownParent(last_parent_hash))?;
				}
				Some(next) => {
					chain.push_front(next);
				}
			}
		}

		// Get the state for last checkpoint.
		let last_checkpoint_hash = *chain
			.front()
			.expect("chain has at least one element; qed")
			.parent_hash();

		let last_checkpoint_header = match Storage::header(last_checkpoint_hash) {
			None => return Err(Error::MissingCheckpoint(last_checkpoint_hash))?,
			Some(header) => header,
		};

		let last_checkpoint_state = match snapshot_by_hash.get_mut(&last_checkpoint_hash) {
			Some(state) => state.clone(),
			None => self.recover_from(&last_checkpoint_header)?,
		};

		snapshot_by_hash.insert(last_checkpoint_header.hash(), last_checkpoint_state.clone());

		// Backfill!
		let mut new_state = last_checkpoint_state.clone();
		for item in chain {
			new_state.apply(item, false)?;
		}
		new_state.calc_next_timestamp(header.timestamp(), clique_variant_config.period)?;
		snapshot_by_hash.insert(header.hash(), new_state.clone());

		let elapsed = backfill_start.elapsed();
		log::trace!(target: "snapshot", "Back-filling succeed, took {} ms.", elapsed.as_millis());
		Ok(new_state)
	}

	// see https://github.com/ethereum/go-ethereum/blob/master/consensus/clique/clique.go#L474
	fn verify(&self, header: &Header) -> Result<Address, Error> {
		let creator = recover_creator(header)?.clone();

		// The signer is not authorized
		if !self.signers.contains(&creator) {
			log::trace!(target: "snapshot", "current state: {}", self);
			Err(Error::NotAuthorized(creator))?
		}

		// The signer has signed a block too recently
		if self.recent_signers.contains(&creator) {
			log::trace!(target: "snapshot", "current state: {}", self);
			Err(Error::TooRecentlySigned(creator))?
		}

		// Wrong difficulty
		let inturn = self.is_inturn(header.number, &creator);

		if inturn && *header.difficulty != DIFF_INTURN {
			Err(Error::InvalidDifficulty(Mismatch {
				expect: DIFF_INTURN,
				found: *header.difficulty,
			}))?
		}

		if !inturn && *header.difficulty() != DIFF_NOTURN {
			Err(Error::InvalidDifficulty(Mismatch {
				expect: DIFF_NOTURN,
				found: *header.difficulty,
			}))?
		}

		Ok(creator)
	}

	/// Verify and apply a new header to current state
	pub fn apply(&mut self, header: &Header, is_checkpoint: bool) -> Result<Address, Error> {
		let creator = self.verify(header)?;
		self.recent_signers.push_front(creator);
		self.rotate_recent_signers();

		if is_checkpoint {
			// checkpoint block should not affect previous tallying, so we check that.
			let signers = extract_signers(header)?;
			if self.signers != signers {
				let invalid_signers: Vec<String> = signers
					.into_iter()
					.filter(|s| !self.signers.contains(s))
					.map(|s| format!("{}", s))
					.collect();
				Err(Error::CliqueFaultyRecoveredSigners(invalid_signers))?
			};
		}

		Ok(creator)
	}

	/// Calculate the next timestamp for `inturn` and `noturn` fails if any of them can't be represented as
	/// `SystemTime`
	// TODO(niklasad1): refactor this method to be in constructor of `Snapshot` instead.
	// This is a quite bad API because we must mutate both variables even when already `inturn` fails
	// That's why we can't return early and must have the `if-else` in the end
	pub fn calc_next_timestamp(&mut self, timestamp: u64, period: u64) -> Result<(), Error> {
		let inturn = timestamp.saturating_add(period);

		self.next_timestamp_inturn = inturn;

		let delay = Duration::from_millis(
			rand::thread_rng().gen_range(0u64, (self.signers.len() as u64 / 2 + 1) * SIGNING_DELAY_NOTURN_MS),
		);
		self.next_timestamp_noturn = inturn.map(|inturn| inturn + delay);

		if self.next_timestamp_inturn.is_some() && self.next_timestamp_noturn.is_some() {
			Ok(())
		} else {
			Err(Error::TimestampOverflow)?
		}
	}

	/// Returns true if the block difficulty should be `inturn`
	pub fn is_inturn(&self, current_block_number: u64, author: &Address) -> bool {
		if let Some(pos) = self.signers.iter().position(|x| *author == *x) {
			return current_block_number % self.signers.len() as u64 == pos as u64;
		}
		false
	}

	/// Returns whether the signer is authorized to sign a block
	pub fn is_authorized(&self, author: &Address) -> bool {
		self.signers.contains(author) && !self.recent_signers.contains(author)
	}

	/// Returns the list of current signers
	pub fn signers(&self) -> &BTreeSet<Address> {
		&self.signers
	}

	fn rotate_recent_signers(&mut self) {
		if self.recent_signers.len() >= (self.signers.len() / 2) + 1 {
			self.recent_signers.pop_back();
		}
	}
}
