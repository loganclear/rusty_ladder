
use super::{config, Config, Error, FromStr};
use crate::ActionCommons;
use ladder_lib::{router, server};
use log::LevelFilter;
use url::Url;

const DEFAULT_LOG_LEVEL: LevelFilter = LevelFilter::Info;

/// Make [`Config`] with arguments like `--inbound`, `--outbound`.
pub(super) fn make_config_from_args(
	in_url: &str,
	out_url: &str,
    allow_lan: bool,
    block_list: &[String],
	coms: ActionCommons,
) -> Result<Config, Error> {
	let rules = make_blocklist(allow_lan, block_list.iter().map(String::as_str))?;
	let inbound = {
		let url = Url::from_str(in_url).map_err(Error::input)?;
		server::inbound::Builder::parse_url(&url).map_err(Error::input)?
	};
	let outbound = {
		if out_url == "freedom" {
			server::outbound::Builder::new_freedom()
		} else {
			let url = Url::from_str(in_url).map_err(Error::input)?;
			server::outbound::Builder::parse_url(&url).map_err(Error::input)?
		}
	};

	let config = Config {
		log: config::Log {
			level: coms.log.unwrap_or(DEFAULT_LOG_LEVEL),
			output: coms.log_out,
		},
		server: server::Builder {
			inbounds: vec![inbound],
			outbounds: vec![outbound],
			router: router::Builder { rules },
			..Default::default()
		},
	};

	Ok(config)
}

fn make_blocklist<'a>(
	allow_lan: bool,
	block_list: impl IntoIterator<Item = &'a str>,
) -> Result<Vec<ladder_lib::router::PlainRule>, Error> {
	const LAN_STR: &str = "@lan";
	const LOCALHOST_STR: &str = "@localhost";

	use ladder_lib::protocol::socks_addr::DomainName;
	use router::{Cidr, Destination};

	fn push_lan(buf: &mut Vec<Destination>) {
		buf.extend(
			Vec::from(Cidr::private_networks())
				.into_iter()
				.map(Destination::Cidr),
		);
	}

	fn push_localhost(buf: &mut Vec<Destination>) {
		buf.push(Destination::Cidr(router::Cidr4::LOCALLOOP.into()));
		buf.push(Destination::Cidr(router::Cidr6::LOCALLOOP.into()));
	}

	let mut dsts = Vec::<Destination>::with_capacity(16);

	if !allow_lan {
		push_lan(&mut dsts);
		push_localhost(&mut dsts);
	}

	for part in block_list {
		match part {
			LAN_STR => push_lan(&mut dsts),
			LOCALHOST_STR => push_localhost(&mut dsts),
			_ => {
				if let Ok(ip) = std::net::IpAddr::from_str(part) {
					dsts.push(Destination::Ip(ip));
				} else if let Ok(name) = DomainName::from_str(part) {
					dsts.push(Destination::Domain(name));
				} else if let Ok(cidr) = Cidr::from_str(part) {
					dsts.push(Destination::Cidr(cidr));
				} else {
					return Err(Error::input(format!(
						"'{}' is not a valid IP or domain",
						part
					)));
				}
			}
		}
	}

	dsts.shrink_to_fit();
	Ok(if dsts.is_empty() {
		Vec::new()
	} else {
		vec![router::PlainRule {
			dsts,
			outbound_tag: None,
			..Default::default()
		}]
	})
}
