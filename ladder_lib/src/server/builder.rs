/**********************************************************************

Copyright (C) 2021 by reddal

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program.  If not, see <https://www.gnu.org/licenses/>.

**********************************************************************/

use super::{inbound, outbound, Api, Server};
use crate::{prelude::*, router};
use std::{collections::HashMap, time::Duration};

const KB: usize = 1024;

const DEFAULT_DIAL_TCP_TIMEOUT_MS: u64 = 10_000;
const DEFAULT_OUTBOUND_HANDSHAKE_TIMEOUT_MS: u64 = 20_000;
const DEFAULT_RELAY_BUFFER_SIZE_KB: usize = 16;
const DEFAULT_RELAY_TIMEOUT_SECS: usize = 300;

#[cfg(feature = "dns")]
use super::dns;

#[derive(Debug, thiserror::Error)]
pub enum BuildError {
	#[error("tag '{tag}' on inbound '{ind}' already exists")]
	InboundTagAlreadyExists { ind: usize, tag: Tag },
	#[error("tag '{tag}' on outbound '{ind}' already exists")]
	OutboundTagAlreadyExists { ind: usize, tag: Tag },
	#[error("error on inbound '{ind}' ({err})")]
	Inbound { ind: usize, err: BoxStdErr },
	#[error("error on outbound '{ind}' ({err})")]
	Outbound { ind: usize, err: BoxStdErr },
	#[error("router error ({0})")]
	Router(#[from] router::Error),
	#[error("api error ({0})")]
	Api(BoxStdErr),
	#[error("value of '{0}' cannot be zero")]
	ValueIsZero(Cow<'static, str>),
}

#[cfg(feature = "use-udp")]
const fn default_udp_session_timeout_ms() -> u64 {
	20_000
}

#[derive(Debug, Default)]
#[cfg_attr(
	feature = "use_serde",
	derive(serde::Deserialize),
	serde(deny_unknown_fields)
)]
pub struct Builder {
	pub inbounds: Vec<inbound::Builder>,
	pub outbounds: Vec<outbound::Builder>,
	#[cfg_attr(feature = "use_serde", serde(default))]
	pub router: router::Builder,
	#[cfg_attr(feature = "use_serde", serde(default))]
	pub api: Api,
	#[cfg(feature = "dns")]
	#[cfg_attr(feature = "use_serde", serde(default))]
	pub dns: Option<dns::Config>,

	/// TCP connection will be dropped if it cannot be established within
	/// this amount of time.
	///
	/// Default: 10000
	#[cfg_attr(feature = "use_serde", serde(default))]
	pub dial_tcp_timeout_ms: Option<u64>,
	/// Outbound handshake will be dropped if it cannot be completed within
	/// this amount of time.
	///
	/// Default: 20000
	#[cfg_attr(feature = "use_serde", serde(default))]
	pub outbound_handshake_timeout_ms: Option<u64>,
	/// Buffer size for relaying.
	///
	/// Default: 16
	#[cfg_attr(feature = "use_serde", serde(default))]
	pub relay_buffer_size_kb: Option<usize>,
	/// Session will be dropped if there are no bytes transferred within
	/// this amount of time.
	///
	/// Defaults: 300
	#[cfg_attr(feature = "use_serde", serde(default))]
	pub relay_timeout_secs: Option<usize>,
	/// Udp socket/tunnel session will be dropped if there is no read or write for more than
	/// this amount of time.
	///
	/// Defaults: 20000
	#[cfg(feature = "use-udp")]
	#[cfg_attr(
		feature = "use_serde",
		serde(default = "default_udp_session_timeout_ms")
	)]
	pub udp_session_timeout_ms: u64,
}

impl Builder {
	/// Creates a [`Server`].
	///
	/// # Errors
	///
	/// Returns an error if any of the inbounds/outbounds or router failed to build.
	pub fn build(self) -> Result<Server, BuildError> {
		type Map = HashMap<Tag, usize>;
		debug!("Server config: {:?}", self);
		// Returns false if tag already exists.
		// Empty tag will be ignored.
		fn add_tag(ind: usize, tag: &Tag, map: &mut Map, other_map: &Map) -> bool {
			tag.is_empty()
				|| (map.insert(tag.clone(), ind).is_none() && other_map.get(tag).is_none())
		}

		let mut inbound_tags = HashMap::new();
		let mut outbound_tags = HashMap::new();

		for (ind, inbound) in self.inbounds.iter().enumerate() {
			if !add_tag(ind, &inbound.tag, &mut inbound_tags, &outbound_tags) {
				return Err(BuildError::InboundTagAlreadyExists {
					ind,
					tag: inbound.tag.clone(),
				});
			}
		}

		for (ind, outbound) in self.outbounds.iter().enumerate() {
			if !add_tag(ind, &outbound.tag, &mut outbound_tags, &inbound_tags) {
				return Err(BuildError::OutboundTagAlreadyExists {
					ind,
					tag: outbound.tag.clone(),
				});
			}
		}

		let router = {
			let find_inbound = |tag: &str| -> Option<usize> { inbound_tags.get(tag).copied() };
			let find_outbound = |tag: &str| -> Option<usize> { outbound_tags.get(tag).copied() };
			self.router.build(find_inbound, find_outbound)?
		};

		let mut inbounds = Vec::with_capacity(self.inbounds.len());
		for (ind, builder) in self.inbounds.into_iter().enumerate() {
			inbounds.push(
				builder
					.build()
					.map_err(|err| BuildError::Inbound { ind, err })?,
			);
		}

		let mut outbounds = Vec::with_capacity(self.outbounds.len());
		for (ind, builder) in self.outbounds.into_iter().enumerate() {
			outbounds.push(
				builder
					.build()
					.map_err(|err| BuildError::Outbound { ind, err })?,
			);
		}

		if let Api::WebApi { secret, addr: _ } = &self.api {
			if secret.is_empty() {
				return Err(BuildError::Api("web API secret cannot be empty".into()));
			}
		}

		let dial_tcp_timeout = Duration::from_millis(if let Some(val) = self.dial_tcp_timeout_ms {
			check_zero(val, "dial_tcp_timeout_ms")?
		} else {
			DEFAULT_DIAL_TCP_TIMEOUT_MS
		});

		let outbound_handshake_timeout =
			Duration::from_millis(if let Some(val) = self.outbound_handshake_timeout_ms {
				check_zero(val, "outbound_handshake_timeout_ms")?
			} else {
				DEFAULT_OUTBOUND_HANDSHAKE_TIMEOUT_MS
			});

		let relay_buffer_size = if let Some(val) = self.relay_buffer_size_kb {
			check_zero_usize(val, "relay_buffer_size_kb")?
		} else {
			DEFAULT_RELAY_BUFFER_SIZE_KB
		} * KB;

		let relay_timeout_secs = if let Some(val) = self.relay_timeout_secs {
			check_zero_usize(val, "relay_timeout_secs")?
		} else {
			DEFAULT_RELAY_TIMEOUT_SECS
		};

		#[cfg(feature = "use-udp")]
		check_zero(self.udp_session_timeout_ms, "udp_session_timeout_ms")?;

		Ok(Server {
			inbounds,
			outbounds,
			router,
			api: self.api,
			#[cfg(feature = "dns")]
			dns: self.dns,
			inbound_tags,
			outbound_tags,
			dial_tcp_timeout,
			outbound_handshake_timeout,
			relay_buffer_size,
			relay_timeout_secs,
			#[cfg(feature = "use-udp")]
			udp_session_timeout: Duration::from_millis(self.udp_session_timeout_ms),
		})
	}
}

#[inline]
fn check_zero_usize(val: usize, val_name: &'static str) -> Result<usize, BuildError> {
	check_zero(val as u64, val_name).map(|_| val)
}

#[inline]
fn check_zero(val: u64, val_name: &'static str) -> Result<u64, BuildError> {
	if val > 0 {
		Ok(val)
	} else {
		Err(BuildError::ValueIsZero(Cow::Borrowed(val_name)))
	}
}
