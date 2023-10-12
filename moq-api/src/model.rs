use serde::{Deserialize, Serialize};

use url::Url;

#[derive(Serialize, Deserialize)]
pub struct Origin {
	pub url: Url,
}
