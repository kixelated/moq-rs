use url::Url;

#[derive(Clone)]
pub struct Api {
	client: moq_api::Client,
	origin: moq_api::Origin,
}

impl Api {
	pub fn new(url: Url, node: Url) -> Self {
		let origin = moq_api::Origin { url: node };
		let client = moq_api::Client::new(url);

		Self { client, origin }
	}

	pub async fn set_origin(&self, broadcast: String) -> Result<Refresh, moq_api::ApiError> {
		let refresh = Refresh::new(self.client.clone(), self.origin.clone(), broadcast);
		refresh.update().await?;
		Ok(refresh)
	}

	pub async fn get_origin(&self, broadcast: &str) -> Result<Option<moq_api::Origin>, moq_api::ApiError> {
		self.client.get_origin(broadcast).await
	}
}

pub struct Refresh {
	client: moq_api::Client,
	origin: moq_api::Origin,
	broadcast: String,
	refresh: tokio::time::Interval,
}

impl Refresh {
	fn new(client: moq_api::Client, origin: moq_api::Origin, broadcast: String) -> Self {
		let duration = tokio::time::Duration::from_secs(300);
		let mut refresh = tokio::time::interval(tokio::time::Duration::from_secs(300));
		refresh.reset_after(duration); // skip the first tick

		Self {
			client,
			origin,
			broadcast,
			refresh,
		}
	}

	async fn update(&self) -> Result<(), moq_api::ApiError> {
		// Register the origin in moq-api.
		log::debug!(
			"registering origin: broadcast={} url={}",
			self.broadcast,
			self.origin.url
		);
		self.client.set_origin(&self.broadcast, self.origin.clone()).await
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		loop {
			self.refresh.tick().await;
			self.update().await?;
		}
	}
}

impl Drop for Refresh {
	fn drop(&mut self) {
		// TODO this is really lazy
		let broadcast = self.broadcast.clone();
		let client = self.client.clone();
		log::debug!("removing origin: broadcast={}", broadcast,);
		tokio::spawn(async move { client.delete_origin(&broadcast).await });
	}
}
