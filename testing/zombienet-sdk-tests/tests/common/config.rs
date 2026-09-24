// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! The `subxt` [`Config`] every test client is built on.
//!
//! # Why not `PolkadotConfig`
//!
//! A chain's metadata can describe several *transaction extension* pipelines, keyed by version.
//! Asset Hub Polkadot does since Fellows runtimes `v2.5.0`: version 0 is the classic list, version
//! 1 adds `AsPgas`, `AsDotnsGateway` and `RestrictOrigins` (from `individuality-community`) plus
//! `VerifyMultiSignature`, and is the pipeline a *general* (`v5`) transaction selects.
//!
//! `subxt` builds a **`v4` signed** extrinsic whenever the chain supports extrinsic version 4 —
//! which Asset Hub Polkadot does — but its [`DefaultExtrinsicParams`] resolves the extension list
//! to the *highest* version in the metadata. In polkadot-sdk a `v4` extrinsic is
//! `Preamble::Signed(Address, Signature, ExtensionV0)`, so the runtime decodes it with **version
//! 0**. The two disagree, and building a transaction fails outright:
//!
//! ```text
//! Cannot construct the required transaction extensions:
//! The chain expects a signed extension with the name AsPgas, but we did not provide one
//! ```
//!
//! Supplying the version-1 extensions would not help: it would produce a version-1 layout inside a
//! `v4` envelope the runtime reads as version 0.
//!
//! # What this does
//!
//! [`V4SignedExtrinsicParams`] is `subxt`'s [`DefaultExtrinsicParams`] with the extension list
//! pinned to the version a `v4` signed extrinsic implies. Every entry of that list is an extension
//! `subxt` already knows, so no custom extension types are needed. On a chain that describes only
//! one pipeline (every other chain these tests touch) it is byte-for-byte identical to
//! [`PolkadotConfig`].

use scale_info::{PortableRegistry, TypeDef};
use std::collections::HashMap;
use subxt::{
	client::ClientState,
	config::{
		transaction_extensions::{
			ChargeAssetTxPayment, ChargeTransactionPayment, CheckGenesis, CheckMetadataHash,
			CheckMortality, CheckNonce, CheckSpecVersion, CheckTxVersion, VerifySignature,
		},
		Config, DefaultExtrinsicParams, ExtrinsicParams, ExtrinsicParamsEncoder,
		ExtrinsicParamsError, PolkadotConfig, TransactionExtension,
	},
};

/// The transaction-extension version a `v4` signed extrinsic is decoded with: `Preamble::Signed`
/// names `ExtensionV0` explicitly, so it is always 0, whatever else the metadata offers.
const EXTENSION_VERSION_FOR_V4: u8 = 0;

/// The `subxt` config used for every chain in these tests: [`PolkadotConfig`]'s types, with
/// [`V4SignedExtrinsicParams`] in place of the default extension handling.
pub enum TestConfig {}

impl Config for TestConfig {
	type AccountId = <PolkadotConfig as Config>::AccountId;
	type Address = <PolkadotConfig as Config>::Address;
	type Signature = <PolkadotConfig as Config>::Signature;
	type Hasher = <PolkadotConfig as Config>::Hasher;
	type Header = <PolkadotConfig as Config>::Header;
	type AssetId = <PolkadotConfig as Config>::AssetId;
	type ExtrinsicParams = V4SignedExtrinsicParams<Self>;
}

type BoxedEncoder = Box<dyn ExtrinsicParamsEncoder + Send + 'static>;

/// `subxt`'s [`DefaultExtrinsicParams`] with the extension list pinned to
/// [`EXTENSION_VERSION_FOR_V4`].
///
/// This mirrors what `subxt`'s `AnyOf` does — offer each extension it knows to the metadata, in
/// metadata order, skipping anything that encodes to zero bytes — except that the list comes from
/// one fixed version rather than the highest one. Params are `subxt`'s own, so
/// `DefaultExtrinsicParamsBuilder` still builds them.
pub struct V4SignedExtrinsicParams<T: Config> {
	encoders: Vec<BoxedEncoder>,
	_marker: core::marker::PhantomData<T>,
}

impl<T: Config> ExtrinsicParams<T> for V4SignedExtrinsicParams<T> {
	type Params = <DefaultExtrinsicParams<T> as ExtrinsicParams<T>>::Params;

	fn new(client: &ClientState<T>, params: Self::Params) -> Result<Self, ExtrinsicParamsError> {
		let metadata = &client.metadata;
		let types = metadata.types();

		// Pinning to version 0 is only right while `subxt` builds `v4` extrinsics for this chain,
		// which it does as long as the chain accepts them. Fail loudly rather than silently sign
		// the wrong layout if that ever stops being true.
		if !metadata.extrinsic().supported_versions().contains(&4) {
			return Err(ExtrinsicParamsError::custom(
				"chain no longer supports v4 extrinsics: this config's extension-version pin is \
				 stale, see the module docs",
			));
		}

		let extensions: Vec<(String, u32)> = metadata
			.extrinsic()
			.transaction_extensions_by_version(EXTENSION_VERSION_FOR_V4)
			.ok_or_else(|| {
				ExtrinsicParamsError::custom(format!(
					"chain describes no version-{EXTENSION_VERSION_FOR_V4} transaction extensions"
				))
			})?
			.map(|e| (e.identifier().to_owned(), e.extra_ty()))
			.collect();

		// Each extension we know claims the first slot it matches and is built with its own params,
		// so every element of the params tuple is consumed exactly once.
		let mut claimed: HashMap<usize, BoxedEncoder> = HashMap::new();
		macro_rules! claim_slot_for {
			($ext:ty, $params:expr) => {{
				for (index, (identifier, extra_ty)) in extensions.iter().enumerate() {
					if claimed.contains_key(&index) {
						continue;
					}
					if <$ext as TransactionExtension<T>>::matches(identifier, *extra_ty, types) {
						let ext = <$ext as ExtrinsicParams<T>>::new(client, $params)?;
						claimed.insert(index, Box::new(ext) as BoxedEncoder);
						break;
					}
				}
			}};
		}
		claim_slot_for!(VerifySignature<T>, params.0);
		claim_slot_for!(CheckSpecVersion, params.1);
		claim_slot_for!(CheckTxVersion, params.2);
		claim_slot_for!(CheckNonce, params.3);
		claim_slot_for!(CheckGenesis<T>, params.4);
		claim_slot_for!(CheckMortality<T>, params.5);
		claim_slot_for!(ChargeAssetTxPayment<T>, params.6);
		claim_slot_for!(ChargeTransactionPayment, params.7);
		claim_slot_for!(CheckMetadataHash, params.8);

		let mut encoders = Vec::with_capacity(extensions.len());
		for (index, (identifier, extra_ty)) in extensions.iter().enumerate() {
			match claimed.remove(&index) {
				Some(encoder) => encoders.push(encoder),
				// Extensions carrying no data need nothing from us.
				None if is_type_empty(*extra_ty, types) => {},
				None =>
					return Err(ExtrinsicParamsError::UnknownTransactionExtension(
						identifier.clone(),
					)),
			}
		}

		Ok(Self { encoders, _marker: core::marker::PhantomData })
	}
}

impl<T: Config> ExtrinsicParamsEncoder for V4SignedExtrinsicParams<T> {
	fn encode_value_to(&self, v: &mut Vec<u8>) {
		for encoder in &self.encoders {
			encoder.encode_value_to(v);
		}
	}

	fn encode_signer_payload_value_to(&self, v: &mut Vec<u8>) {
		for encoder in &self.encoders {
			encoder.encode_signer_payload_value_to(v);
		}
	}

	fn encode_implicit_to(&self, v: &mut Vec<u8>) {
		for encoder in &self.encoders {
			encoder.encode_implicit_to(v);
		}
	}

	fn inject_signature(
		&mut self,
		account_id: &dyn core::any::Any,
		signature: &dyn core::any::Any,
	) {
		for encoder in &mut self.encoders {
			encoder.inject_signature(account_id, signature);
		}
	}
}

/// Whether a type encodes to zero bytes, i.e. an extension we can leave out. Mirrors the check
/// `subxt` applies to the extensions it doesn't recognize.
fn is_type_empty(type_id: u32, types: &PortableRegistry) -> bool {
	let Some(ty) = types.resolve(type_id) else {
		// Unresolvable, so we can't claim it is empty.
		return false;
	};
	match &ty.type_def {
		TypeDef::Composite(c) => c.fields.iter().all(|f| is_type_empty(f.ty.id, types)),
		TypeDef::Array(a) => a.len == 0 || is_type_empty(a.type_param.id, types),
		TypeDef::Tuple(t) => t.fields.iter().all(|f| is_type_empty(f.id, types)),
		TypeDef::BitSequence(_) |
		TypeDef::Variant(_) |
		TypeDef::Sequence(_) |
		TypeDef::Compact(_) |
		TypeDef::Primitive(_) => false,
	}
}
