// Copyright (C) Parity Technologies (UK) Ltd.
// SPDX-License-Identifier: Apache-2.0

//! Reusable, bridge-pair-agnostic test infrastructure (the "zombienet-sdk-tests-lib"): generic
//! `subxt` helpers, the `substrate-relay` subprocess driver and the default node images. A bridge
//! pair (e.g. `rococo_westend`) builds its environment on top of these.

pub mod images;
pub mod relayer;
pub mod utils;
