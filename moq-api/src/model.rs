use serde::{Deserialize, Serialize};

use url::Url;

#[derive(Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct Origin {
	pub url: Url,
}
