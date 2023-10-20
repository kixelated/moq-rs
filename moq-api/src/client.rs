use url::Url;

use crate::{ApiError, Origin};

#[derive(Clone)]
pub struct Client {
	// The address of the moq-api server
	url: Url,

	client: reqwest::Client,
}

impl Client {
	pub fn new(url: Url) -> Self {
		let client = reqwest::Client::new();
		Self { url, client }
	}

	pub async fn get_origin(&self, id: &str) -> Result<Option<Origin>, ApiError> {
		let url = self.url.join("origin/")?.join(id)?;
		let resp = self.client.get(url).send().await?;
		if resp.status() == reqwest::StatusCode::NOT_FOUND {
			return Ok(None);
		}

		let origin: Origin = resp.json().await?;
		Ok(Some(origin))
	}

	pub async fn set_origin(&mut self, id: &str, origin: &Origin) -> Result<(), ApiError> {
		let url = self.url.join("origin/")?.join(id)?;

		let resp = self.client.post(url).json(origin).send().await?;
		resp.error_for_status()?;

		Ok(())
	}

	pub async fn delete_origin(&mut self, id: &str) -> Result<(), ApiError> {
		let url = self.url.join("origin/")?.join(id)?;

		let resp = self.client.delete(url).send().await?;
		resp.error_for_status()?;

		Ok(())
	}

	pub async fn patch_origin(&mut self, id: &str, origin: &Origin) -> Result<(), ApiError> {
		let url = self.url.join("origin/")?.join(id)?;

		let resp = self.client.patch(url).json(origin).send().await?;
		resp.error_for_status()?;

		Ok(())
	}
}
