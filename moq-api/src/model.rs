use serde::{Deserialize, Serialize};

use url::Url;

#[derive(Serialize, Deserialize, PartialEq, Eq)]
pub struct Origin {
	pub url: Url,
}
