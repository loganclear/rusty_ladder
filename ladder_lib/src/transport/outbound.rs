/**builder****************************************************

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

#[allow(unused_imports)]
use crate::protocol::ProxyContext;

#[ladder_lib_macro::impl_variants(Outbound)]
mod settings {
	use crate::protocol::{AsyncReadWrite, ProxyContext, SocksAddr};
	use std::io;
	use tokio::io::{AsyncRead, AsyncWrite};

	pub enum Outbound {
		#[cfg(any(feature = "tls-transport-openssl", feature = "tls-transport-rustls"))]
		Tls(super::super::tls::Outbound),
		#[cfg(any(feature = "ws-transport-openssl", feature = "ws-transport-rustls"))]
		Ws(super::super::ws::Outbound),
		#[cfg(any(feature = "h2-transport-openssl", feature = "h2-transport-rustls"))]
		H2(super::super::h2::Outbound),
		#[cfg(feature = "browser-transport")]
		Browser(super::super::browser::Settings),
	}

	impl Outbound {
		#[implement(map_into)]
		pub async fn connect_stream<'a, IO>(
			&'a self,
			stream: IO,
			#[allow(unused_variables)] addr: &'a SocksAddr,
		) -> io::Result<Box<dyn AsyncReadWrite>>
		where
			IO: 'static
				+ AsyncRead
				+ AsyncWrite
				+ Unpin
				+ Send
				+ Sync
				+ Into<Box<dyn AsyncReadWrite>>,
		{
		}
		#[implement(map_into)]
		pub async fn connect(
			&self,
			addr: &SocksAddr,
			context: &dyn ProxyContext,
		) -> io::Result<Box<dyn AsyncReadWrite>> {
		}
	}
}

pub use settings::Outbound;

#[ladder_lib_macro::impl_variants(Builder)]
mod builder {
	use super::Outbound;
	use crate::prelude::BoxStdErr;

	#[cfg_attr(test, derive(PartialEq, Eq))]
	#[derive(Clone, Debug)]
	#[cfg_attr(
		feature = "use_serde",
		derive(serde::Deserialize),
		serde(rename_all = "lowercase", tag = "type")
	)]
	pub enum Builder {
		#[cfg(any(feature = "tls-transport-openssl", feature = "tls-transport-rustls"))]
		Tls(super::super::tls::OutboundBuilder),
		#[cfg(any(feature = "ws-transport-openssl", feature = "ws-transport-rustls"))]
		Ws(super::super::ws::OutboundBuilder),
		#[cfg(any(feature = "h2-transport-openssl", feature = "h2-transport-rustls"))]
		H2(super::super::h2::OutboundBuilder),
		#[cfg(feature = "browser-transport")]
		Browser(super::super::browser::SettingsBuilder),
	}

	impl Builder {
		#[implement(map_into_map_err_into)]
		pub fn build(self) -> Result<Outbound, BoxStdErr> {}
	}

	impl crate::protocol::DisplayInfo for Builder {
		#[implement(map_into)]
		fn fmt_brief(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {}
		#[implement(map_into)]
		fn fmt_detail(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {}
	}
}
pub use builder::Builder;
