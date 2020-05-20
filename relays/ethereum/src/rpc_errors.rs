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

#![allow(dead_code)]

use jsonrpsee::raw::client::RawClientError;
use jsonrpsee::transport::http::RequestError;
use serde_json;

type RpcHttpError = RawClientError<RequestError>;

#[derive(Debug)]
pub enum RpcError {
	Serialization(serde_json::Error),
	Ethereum(EthereumNodeError),
	Substrate(SubstrateNodeError),
	Request(RpcHttpError),
	Decoding(codec::Error),
}

impl From<serde_json::Error> for RpcError {
	fn from(err: serde_json::Error) -> Self {
		Self::Serialization(err)
	}
}

impl From<EthereumNodeError> for RpcError {
	fn from(err: EthereumNodeError) -> Self {
		Self::Ethereum(err)
	}
}

impl From<SubstrateNodeError> for RpcError {
	fn from(err: SubstrateNodeError) -> Self {
		Self::Substrate(err)
	}
}

impl From<RpcHttpError> for RpcError {
	fn from(err: RpcHttpError) -> Self {
		Self::Request(err)
	}
}

impl From<codec::Error> for RpcError {
	fn from(err: codec::Error) -> Self {
		Self::Decoding(err)
	}
}

#[derive(Debug)]
pub enum EthereumNodeError {
	/// Failed to parse response.
	ResponseParseFailed(String),
	/// We have received header with missing number and hash fields.
	IncompleteHeader,
	/// We have received receipt with missing gas_used field.
	IncompleteReceipt,
	/// Invalid Substrate block number received from Ethereum node.
	InvalidSubstrateBlockNumber,
}

#[derive(Debug)]
pub enum SubstrateNodeError {
	/// Request start failed.
	StartRequestFailed(RequestError),
	/// Error serializing request.
	RequestSerialization(serde_json::Error),
	/// Failed to parse response.
	ResponseParseFailed,
}
