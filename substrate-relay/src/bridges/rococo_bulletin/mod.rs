// Copyright 2019-2021 Parity Technologies (UK) Ltd.
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

//! Declaration of all bridges between Rococo Bulletin Chain and Rococo Bridge Hub.

pub mod bridge_hub_rococo_messages_to_rococo_bulletin;
pub mod rococo_bulletin_headers_to_bridge_hub_rococo;
pub mod rococo_bulletin_messages_to_bridge_hub_rococo;
pub mod rococo_headers_to_rococo_bulletin;
pub mod rococo_parachains_to_rococo_bulletin;
