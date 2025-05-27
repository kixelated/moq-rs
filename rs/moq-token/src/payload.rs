use anyhow::Context;
use clap::Args;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, TimestampSeconds};

#[serde_as]
#[derive(Debug, Serialize, Deserialize, Args)]
pub struct Payload {
	/// The root path. Any paths are relative to this path.
	/// It is not legal to use something like ../ to escape the root path.
	#[arg(long)]
	root: Option<String>,

	/// If specified, the user can publish any broadcasts matching a prefix.
	/// One or more prefixes can be specified with multiple --publish flags.
	#[arg(long)]
	#[serde(rename = "pub")]
	publish: Vec<String>,

	/// If specified, the user will publish this path.
	/// No announcement is needed, and the broadcast is considered active while the connection is active.
	/// This is useful to avoid an RTT and informs all other clients that this user is connected.
	#[arg(long)]
	#[serde(rename = "pubx")]
	publish_force: Vec<String>,

	/// If specified, the user can subscribe to any broadcasts matching a prefix.
	/// One or more prefixes can be specified with multiple --subscribe flags.
	#[arg(long)]
	#[serde(rename = "sub")]
	subscribe: Vec<String>,

	/// The expiration time of the token as a unix timestamp.
	#[arg(long, value_parser = parse_unix_timestamp)]
	#[serde(rename = "exp")]
	#[serde_as(as = "Option<TimestampSeconds<i64>>")]
	expires: Option<std::time::SystemTime>,

	/// The issued time of the token as a unix timestamp.
	#[arg(long, value_parser = parse_unix_timestamp)]
	#[serde(rename = "iat")]
	#[serde_as(as = "Option<TimestampSeconds<i64>>")]
	issued: Option<std::time::SystemTime>,
}

// A simpler parser for clap
fn parse_unix_timestamp(s: &str) -> anyhow::Result<std::time::SystemTime> {
	let timestamp = s.parse::<i64>().context("expected unix timestamp")?;
	let timestamp = timestamp.try_into().context("timestamp out of range")?;
	Ok(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(timestamp))
}
