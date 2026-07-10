// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Default node container images for the `docker`/`podman` zombienet providers.

// Default node images for the `docker` provider, overridable via `POLKADOT_IMAGE` /
// `CUMULUS_IMAGE`. The paritypr `*-debug` images built from polkadot-sdk master are tagged
// `master-<short-8-char-commit>`, so tag with the `Cargo.lock` revision accordingly (see
// `build.rs`).
const DEFAULT_POLKADOT_IMAGE: &str =
	concat!("docker.io/paritypr/polkadot-debug:master-", env!("POLKADOT_SDK_SHORT_HASH"));
const DEFAULT_CUMULUS_IMAGE: &str =
	concat!("docker.io/paritypr/polkadot-parachain-debug:master-", env!("POLKADOT_SDK_SHORT_HASH"));

pub struct NodeImages {
	pub polkadot: String,
	pub cumulus: String,
}

pub fn node_images() -> NodeImages {
	NodeImages {
		polkadot: std::env::var("POLKADOT_IMAGE")
			.unwrap_or_else(|_| DEFAULT_POLKADOT_IMAGE.to_string()),
		cumulus: std::env::var("CUMULUS_IMAGE")
			.unwrap_or_else(|_| DEFAULT_CUMULUS_IMAGE.to_string()),
	}
}
