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

use clap::Parser as ClapParser;
use codec::Encode;
use color_eyre::eyre;
use std::{env, path::PathBuf};
use subxt_codegen::{
	generate_runtime_api_from_bytes, generate_runtime_api_from_url, utils::Uri, CratePath,
	DerivesRegistry, TypeSubstitutes,
};
use wasm_testbed::WasmTestBed;

/// Command for generating indirect runtimes code.
#[derive(Debug, ClapParser)]
struct Command {
	#[clap(name = "from-node-url", long, value_parser)]
	node_url: Option<Uri>,
	#[clap(name = "from-wasm-file", long, value_parser)]
	wasm_file: Option<String>,
}

enum RuntimeMetadataSource {
	NodeUrl(Uri),
	WasmFile(wasm_loader::Source),
}

impl RuntimeMetadataSource {
	fn from_command(cmd: Command) -> color_eyre::Result<Self> {
		match (cmd.node_url, cmd.wasm_file) {
			(Some(_), Some(_)) => Err(eyre::eyre!(
				"Please specify one of `--from-node-url` or `--from-wasm-file` but not both"
			)),
			(None, None) =>
				Err(eyre::eyre!("Please specify one of `--from-node-url` or `--from-wasm-file`")),
			(Some(node_url), None) => Ok(Self::NodeUrl(node_url)),
			(None, Some(source)) =>
				Ok(Self::WasmFile(wasm_loader::Source::File(PathBuf::from(source)))),
		}
	}
}

struct TypeSubstitute {
	subxt_type: syn::Path,
	substitute: syn::Path,
}

impl TypeSubstitute {
	fn simple(subxt_type: &str) -> Self {
		Self {
			subxt_type: syn::parse_str::<syn::Path>(subxt_type).unwrap(),
			substitute: syn::parse_str::<syn::Path>(&format!("::{subxt_type}")).unwrap(),
		}
	}

	fn custom(subxt_type: &str, substitute: &str) -> Self {
		Self {
			subxt_type: syn::parse_str::<syn::Path>(subxt_type).unwrap(),
			substitute: syn::parse_str::<syn::Path>(substitute).unwrap(),
		}
	}
}

fn print_runtime(runtime_api: proc_macro2::TokenStream) {
	println!(
		"// Copyright 2019-2023 Parity Technologies (UK) Ltd.
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

		//! Autogenerated runtime API
		//! THIS FILE WAS AUTOGENERATED USING parity-bridges-common::runtime-codegen
		//! EXECUTED COMMAND: {}

		{}
		",
		env::args().collect::<Vec<String>>().join(" "),
		runtime_api
	);
}

fn main() -> color_eyre::Result<()> {
	let args: Command = Command::parse();
	let metadata_source = RuntimeMetadataSource::from_command(args)?;

	// Module under which the API is generated.
	let item_mod = syn::parse_quote!(
		pub mod api {}
	);
	// Default module derivatives.
	let mut derives = DerivesRegistry::new(&CratePath::default());
	derives.extend_for_all(vec![syn::parse_quote!(Clone)]);
	// Type substitutes
	let mut type_substitutes = TypeSubstitutes::new(&CratePath::default());
	type_substitutes.extend(
		vec![
			TypeSubstitute::simple("sp_core::crypto::AccountId32"),
			TypeSubstitute::custom("bp_millau::millau_hash::MillauHash", "::bp_millau::MillauHash"),
			TypeSubstitute::simple("bp_millau::BlakeTwoAndKeccak256"),
			TypeSubstitute::custom(
				"sp_runtime::generic::digest::Digest",
				"::sp_runtime::generic::Digest",
			),
			TypeSubstitute::custom("sp_runtime::generic::era::Era", "::sp_runtime::generic::Era"),
			TypeSubstitute::custom(
				"sp_runtime::generic::header::Header",
				"::sp_runtime::generic::Header",
			),
			TypeSubstitute::simple("bp_header_chain::justification::GrandpaJustification"),
			TypeSubstitute::simple("bp_header_chain::InitializationData"),
			TypeSubstitute::simple(
				"bridge_runtime_common::messages::target::FromBridgedChainMessagesProof",
			),
			TypeSubstitute::custom("sp_weights::weight_v2::Weight", "::sp_weights::Weight"),
			TypeSubstitute::simple(
				"bridge_runtime_common::messages::source::FromBridgedChainMessagesDeliveryProof",
			),
			TypeSubstitute::simple("bp_messages::UnrewardedRelayersState"),
		]
		.drain(..)
		.map(|substitute| (substitute.subxt_type, substitute.substitute.try_into().unwrap())),
	);

	// Generate the Runtime API.
	let runtime_api = match metadata_source {
		RuntimeMetadataSource::NodeUrl(node_url) => generate_runtime_api_from_url(
			item_mod,
			&node_url,
			derives,
			type_substitutes,
			CratePath::default(),
		),
		RuntimeMetadataSource::WasmFile(source) => {
			let testbed = WasmTestBed::new(&source)
				.map_err(|e| eyre::eyre!("Error creating WasmTestBed: {:?}", e))?;
			generate_runtime_api_from_bytes(
				item_mod,
				&testbed.runtime_metadata_prefixed().encode(),
				derives,
				type_substitutes,
				CratePath::default(),
			)
		},
	};

	print_runtime(runtime_api);

	Ok(())
}
