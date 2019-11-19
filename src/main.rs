mod bridge;
mod error;
mod rpc;

use error::Error;
use rpc::SubstrateRPC;

use clap::{App, Arg, value_t, values_t};
use futures::{
	prelude::*,
	future,
};
use jsonrpsee::{
	core::client::{Client, ClientError, ClientEvent},
	ws::{WsClient, WsConnecError, ws_client},
};
use node_primitives::{Hash, Header};
use url::Url;
use std::collections::HashMap;
use std::process;
use std::str::FromStr;

const DEFAULT_WS_PORT: u16 = 9944;

#[derive(Debug, Clone)]
struct RPCUrlParam {
	url: Url,
}

impl ToString for RPCUrlParam {
	fn to_string(&self) -> String {
		self.url.to_string()
	}
}

impl FromStr for RPCUrlParam {
	type Err = Error;

	fn from_str(url_str: &str) -> Result<Self, Self::Err> {
		let mut url = Url::parse(url_str)
			.map_err(|e| Error::UrlError(format!("could not parse {}: {}", url_str, e)))?;

		if url.scheme() != "ws" {
			return Err(Error::UrlError(format!("must have scheme ws, found {}", url.scheme())));
		}

		if url.port().is_none() {
			url.set_port(Some(DEFAULT_WS_PORT))
				.expect("the scheme is checked above to be ws; qed");
		}

		if url.path() != "/" {
			return Err(Error::UrlError(format!("cannot have a path, found {}", url.path())));
		}
		if let Some(query) = url.query() {
			return Err(Error::UrlError(format!("cannot have a query, found {}", query)));
		}
		if let Some(fragment) = url.fragment() {
			return Err(Error::UrlError(format!("cannot have a fragment, found {}", fragment)));
		}

		Ok(RPCUrlParam { url })
	}
}

pub type BridgeId = u32;

struct BridgeState {
	locally_finalized_head_on_bridged_chain: Header,
}

struct ChainState {
	current_finalized_head: Header,
	bridges: HashMap<BridgeId, BridgeState>,
}

struct Chain {
	url: String,
	client: WsClient,
	genesis_hash: Hash,
	state: ChainState,
}

fn main() {
	let params = parse_args();
	env_logger::init();

	if let Err(err) = run(params) {
		log::error!("{}", err);
		process::exit(1);
	}
}

#[derive(Debug, Clone)]
struct Params {
	base_path: String,
	rpc_urls: Vec<RPCUrlParam>,
}

fn parse_args() -> Params {
	let matches = App::new("substrate-bridge")
		.version("1.0")
		.author("Parity Technologies")
		.about("Bridges Substrates, duh")
		.arg(
			Arg::with_name("base-path")
				.long("base-path")
				.value_name("DIRECTORY")
				.required(true)
				.help("Sets the base path")
				.takes_value(true),
		)
		.arg(
			Arg::with_name("rpc-url")
				.long("rpc-url")
				.value_name("HOST[:PORT]")
				.help("The URL of a bridged Substrate node")
				.takes_value(true)
				.multiple(true)
		)
		.get_matches();

	let base_path = value_t!(matches, "base-path", String)
		.unwrap_or_else(|e| e.exit());
	let rpc_urls = matches.values_of("rpc-url")
		.unwrap()
		.map(RPCUrlParam::from_str)
		.collect::<Result<_, _>>()
		.unwrap_or_else(|e| {
			eprintln!("{}", e);
			Vec::new()
		});
	let rpc_urls = values_t!(matches, "rpc-url", RPCUrlParam)
		.unwrap_or_else(|e| e.exit());

	Params {
		base_path,
		rpc_urls,
	}
}

fn run(params: Params) -> Result<(), Error> {
	async_std::task::block_on(async move {
		run_async(params).await
	})
}

async fn init_rpc_connection(url: &RPCUrlParam) -> Result<Chain, Error> {
	let url_str = url.to_string();
	log::debug!("Connecting to {}", url_str);

	// Skip the leading "ws://" and trailing "/".
	let url_without_scheme = &url_str[5..(url_str.len() - 1)];
	let mut client = ws_client(url_without_scheme)
		.await
		.map_err(|err| Error::WsConnectionError(err.to_string()))?;

	let genesis_hash = rpc::genesis_block_hash(&mut client)
		.await
		.map_err(|e| Error::RPCError(e.to_string()))?
		.ok_or_else(|| Error::InvalidChainState(format!(
			"chain with RPC URL {} is missing a genesis block hash",
			url_str,
		)))?;

	let latest_finalized_hash = SubstrateRPC::chain_finalized_head(&mut client)
		.await
		.map_err(|e| Error::RPCError(e.to_string()))?;
	let latest_finalized_header = SubstrateRPC::chain_header(
		&mut client,
		Some(latest_finalized_hash)
	)
		.await
		.map_err(|e| Error::RPCError(e.to_string()))?
		.ok_or_else(|| Error::InvalidChainState(format!(
			"chain {} is missing header for finalized block hash {}",
			genesis_hash, latest_finalized_hash
		)))?;

	Ok(Chain {
		url: url_str,
		client,
		genesis_hash,
		state: ChainState {
			current_finalized_head: latest_finalized_header,
			bridges: HashMap::new(),
		}
	})
}

async fn run_async(params: Params) -> Result<(), Error> {
	let mut chains = future::join_all(params.rpc_urls.iter().map(init_rpc_connection))
		.await
		.into_iter()
		.collect::<Result<Vec<_>, _>>()?;

	for chain in chains {
		println!("Genesis hash: {}", chain.genesis_hash);
	}

	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn rpc_url_from_str() {
		assert_eq!(
			RPCUrlParam::from_str("ws://127.0.0.1").unwrap().to_string(),
			"ws://127.0.0.1:9944/"
		);
		assert_eq!(
			RPCUrlParam::from_str("ws://127.0.0.1/").unwrap().to_string(),
			"ws://127.0.0.1:9944/"
		);
		assert_eq!(
			RPCUrlParam::from_str("ws://127.0.0.1:4499").unwrap().to_string(),
			"ws://127.0.0.1:4499/"
		);
		assert!(RPCUrlParam::from_str("http://127.0.0.1").is_err());
	}
}
