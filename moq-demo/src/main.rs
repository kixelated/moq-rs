use std::{fs, io, net, path, sync::{self, Arc}};

use anyhow::Context;
use async_webtransport_handler::{AsyncWebTransportServer, regex::Regex};
use clap::Parser;
use moq_transport::{Role, SetupServer, Version};
use ring::digest::{digest, SHA256};
use tokio::task::JoinSet;
use warp::Filter;

use moq_warp::{relay::{self, ServerConfig}, source};
use webtransport_quiche::quiche;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser, Clone)]
struct Cli {
	/// Listen on this address
	#[arg(short, long, default_value = "[::]:4443")]
	addr: net::SocketAddr,

	/// Use the certificate file at this path
	#[arg(short, long, default_value = "cert/localhost.crt")]
	cert: path::PathBuf,

	/// Use the private key at this path
	#[arg(short, long, default_value = "cert/localhost.key")]
	key: path::PathBuf,

	/// Use the media file at this path
	#[arg(short, long, default_value = "media/fragmented.mp4")]
	media: path::PathBuf,

	/// use quiche instead of quinn
	#[arg(short, long)]
	quiche: bool,
}


// Create a new server
pub fn new_quiche(config: ServerConfig) -> anyhow::Result<(AsyncWebTransportServer, tokio::net::UdpSocket, Vec<Regex>)> {
	let mut quic_config = quiche::Config::new(quiche::PROTOCOL_VERSION).unwrap();

	println!("loading cert {:?}, key {:?}", config.cert, config.key);
	quic_config.load_cert_chain_from_pem_file(config.cert.to_str().unwrap()).unwrap();
	quic_config.load_priv_key_from_pem_file(config.key.to_str().unwrap()).unwrap();
	quic_config
		.set_application_protos(quiche::h3::APPLICATION_PROTOCOL)
		.unwrap();
	
	quic_config.set_cc_algorithm_name("cubic").unwrap();
	quic_config.set_max_idle_timeout(10000);
	quic_config.set_max_recv_udp_payload_size(1200);
	quic_config.set_max_send_udp_payload_size(1200);
	quic_config.set_initial_max_data(1_000_000_000);
	quic_config.set_initial_max_stream_data_bidi_local(100_000_000);
	quic_config.set_initial_max_stream_data_bidi_remote(100_000_000);
	quic_config.set_initial_max_stream_data_uni(100_000_000);
	quic_config.set_initial_max_streams_bidi(1_000_000);
	quic_config.set_initial_max_streams_uni(1_000_000);
	quic_config.set_disable_active_migration(true);
	quic_config.enable_early_data();
	quic_config.grease(false);
	// quic_config.set_fec_scheduler_algorithm(quiche::FECSchedulerAlgorithm::BurstsOnly);
	// quic_config.send_fec(args.get_bool("--send-fec"));
	// quic_config.receive_fec(args.get_bool("--receive-fec"));
	// quic_config.set_real_time(args.get_bool("--real-time-cc"));
	let h3_config = quiche::h3::Config::new().unwrap();
	
	let keylog = if let Some(keylog_path) = std::env::var_os("SSLKEYLOGFILE") {
		let file = std::fs::OpenOptions::new()
			.create(true)
			.append(true)
			.open(keylog_path)
			.unwrap();

		Some(file)
	} else {
		None
	};

	let (server, socket) = AsyncWebTransportServer::with_configs(config.addr,
		quic_config, h3_config, keylog)?;
	let uri_root = "/";
	let regexes = [Regex::new(format!("{}", uri_root).as_str()).unwrap()];

	Ok((server, socket, regexes.to_vec()))
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	let args = Cli::parse();

	// Create a web server to serve the fingerprint
	let serve = serve_http(args.clone());
	let mut tasks = JoinSet::new();
	tasks.spawn(async move {
		serve.await.unwrap();
	});

	// Create a fake media source from disk.
	let media = source::File::new(args.media).context("failed to open file source")?;

	let broker = relay::broker::Broadcasts::new();
	broker
		.announce("quic.video/demo", media.source())
		.context("failed to announce file source")?;

	let mut tasks = JoinSet::new();
	tasks.spawn(async move {
		media.run().await.unwrap();
	});
	
	// Create a server to actually serve the media
	let config = relay::ServerConfig {
		addr: args.addr,
		cert: args.cert,
		key: args.key,
		broker: broker.clone(),
	};

	if args.quiche {
		let (server, socket, regexes) = new_quiche(config).unwrap();
		let server = Arc::new(std::sync::Mutex::new(server));
		let socket = Arc::new(socket);
		let mut buf = vec![0; 10000];
		let mut tasks = JoinSet::new();
		'mainloop: loop {
			println!("listen...");
			let cid = {
				// let mut server = endpoint.quiche_server.lock().await;
				let ret = async_webtransport_handler::AsyncWebTransportServer::listen_ref(server.clone(), socket.clone(), &mut buf).await?;
				println!("listen returned {:?}", ret);
				match ret {
					Some(cid) => cid,
					None => continue 'mainloop,
				}
			};
			
			loop {
				println!("poll");
				match server.lock().unwrap().poll(&cid, &regexes[..]) {
					Ok(async_webtransport_handler::Event::NewSession(path, session_id, _regex_index)) => {

						let server = server.clone();
						let cid = cid.clone();
						let broker = broker.clone();
						tasks.spawn(async move {
							// let control_stream = async_webtransport_handler::ServerBidiStream::new(server.clone(), cid.clone(), session_id, session_id);
							let mut webtransport_session = async_webtransport_handler::WebTransportSession::new(server.clone(), cid.clone(), session_id);
							let control_stream = moq_generic_transport::accept_bidi(&mut webtransport_session).await.unwrap().unwrap();
							// let control_stream = async_webtransport_handler::ServerBidiStream::new(server.clone(), cid.clone(), session_id, control_stream_id);
							// let session = moq_transport_trait::Session::new(Box::new(control_stream), Box::new(webtransport_session));
							let received_client_setup = moq_transport_trait::Session::accept(Box::new(control_stream), Box::new(webtransport_session)).await.unwrap();
							// TODO: maybe reject setup
							let role = match received_client_setup.setup().role {
								Role::Publisher => Role::Subscriber,
								Role::Subscriber => Role::Publisher,
								Role::Both => Role::Both,
							};
							let setup_server = SetupServer {
								version: Version::DRAFT_00,
								role,
							};
						
							let session = received_client_setup.accept(setup_server).await.unwrap();
							let session = relay::Session::from_session(session, broker.clone()).await.unwrap();
							session.run().await
						});
					},
					Ok(async_webtransport_handler::Event::StreamData(session_id, stream_id)) => {
						log::trace!("new data!");
					},
					Ok(async_webtransport_handler::Event::Done) => {
						println!("H3 Done");
						break;
					},
					Ok(async_webtransport_handler::Event::GoAway) => {
						println!("GOAWAY");
						break;
					},

					Err(_) => todo!(),
				}
			}
		}

		// let session = moq_transport_trait::Session::new(control_stream, connection)
		// let server = relay::Server::new(config).context("failed to create server")?;
		// // Run all of the above
		// tokio::select! {
		// 	res = server.run() => res.context("failed to run server"),
		// 	res = media.run() => res.context("failed to run media source"),
		// 	res = serve => res.context("failed to run HTTP server"),
		// }
	} else {
		// let server = relay::Server::new(config).context("failed to create server")?;
	
		// Run all of the above
		// tokio::select! {
		// 	res = server.run() => res.context("failed to run server"),
		// 	res = media.run() => res.context("failed to run media source"),
		// 	res = serve => res.context("failed to run HTTP server"),
		// }

	}
	Ok(())
}

// Run a HTTP server using Warp
// TODO remove this when Chrome adds support for self-signed certificates using WebTransport
async fn serve_http(args: Cli) -> anyhow::Result<()> {
	// Read the PEM certificate file
	let crt = fs::File::open(&args.cert)?;
	let mut crt = io::BufReader::new(crt);

	// Parse the DER certificate
	let certs = rustls_pemfile::certs(&mut crt)?;
	let cert = certs.first().expect("no certificate found");

	// Compute the SHA-256 digest
	let fingerprint = digest(&SHA256, cert.as_ref());
	let fingerprint = hex::encode(fingerprint.as_ref());
	let fingerprint = sync::Arc::new(fingerprint);

	let cors = warp::cors().allow_any_origin();

	// What an annoyingly complicated way to serve a static String
	// I spent a long time trying to find the exact way of cloning and dereferencing the Arc.
	let routes = warp::path!("fingerprint")
		.map(move || (*(fingerprint.clone())).clone())
		.with(cors);

	warp::serve(routes)
		.tls()
		.cert_path(args.cert)
		.key_path(args.key)
		.run(args.addr)
		.await;

	Ok(())
}
