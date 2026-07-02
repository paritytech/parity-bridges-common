// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Default node container images for the `docker`/`podman` zombienet providers.

// Default node images for the `docker` provider, overridable via `POLKADOT_IMAGE` /
// `CUMULUS_IMAGE`. Tagged with the polkadot-sdk revision pinned in `Cargo.lock` (see `build.rs`).
const DEFAULT_POLKADOT_IMAGE: &str =
	concat!("docker.io/paritypr/polkadot-debug:", env!("POLKADOT_SDK_SHORT_HASH"));
const DEFAULT_CUMULUS_IMAGE: &str =
	concat!("docker.io/paritypr/polkadot-parachain-debug:", env!("POLKADOT_SDK_SHORT_HASH"));

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
