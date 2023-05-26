use codec::{Decode, Encode};
use sp_runtime::traits::Header as HeaderT;
use sp_runtime::RuntimeDebug;

use crate::AuthoritySignature;

#[derive(PartialEq, Eq, Clone, Debug, Hash, Decode, Encode)]
pub struct Signature(AuthoritySignature);

#[derive(Clone, Debug, Eq, Hash, PartialEq, Encode, Decode)]
pub struct SignatureSet<Signature>(pub aleph_bft_crypto::SignatureSet<Signature>);

/// A proof of block finality, currently in the form of a sufficiently long list of signatures or a
/// sudo signature of a block for emergency finalization.
#[derive(Clone, Encode, Decode, Debug, PartialEq, Eq)]
pub enum AlephJustification {
	CommitteeMultisignature(SignatureSet<Signature>),
	EmergencySignature(AuthoritySignature),
}

#[derive(Encode, Decode, RuntimeDebug, Clone, PartialEq, Eq)]
pub struct AlephFullJustification<Header: HeaderT> {
	header: Header,
	justification: AlephJustification,
}

impl<Header: HeaderT> AlephFullJustification<Header> {
	pub fn new(header: Header, justification: AlephJustification) -> Self {
		Self { header, justification }
	}

	pub fn header(&self) -> &Header {
		&self.header
	}

	pub fn justification(&self) -> &AlephJustification {
		&self.justification
	}

	pub fn into_justification(self) -> AlephJustification {
		self.justification
	}

	pub fn into_header(self) -> Header {
		self.header
	}
}

impl<H: HeaderT> bp_header_chain::FinalityProof<H::Number> for AlephFullJustification<H> {
	fn target_header_number(&self) -> H::Number {
		*self.header().number()
	}
}
