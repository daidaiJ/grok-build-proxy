//! LOCAL(insecure-tls): end-to-end coverage for `[network] insecure_tls_hosts`.
//!
//! Two loopback addresses carry the same self-signed certificate; only the
//! allowlisted one is reachable, so the difference under test is the allowlist
//! and nothing else.
//!
//! One test owns this binary: the allowlist latches process-wide at first read,
//! so every phase runs in one ordered test body.

mod support;

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use axum::Router;
use axum::extract::State;
use axum::routing::post;
use rcgen::{CertificateParams, KeyPair};
use support::{send_one, test_config};
use xai_grok_sampler::SamplingClient;

/// Starts a TLS server on `bind` with a self-signed certificate (no CA in any root store).
async fn start_self_signed_server(bind: &str, accepted: Arc<AtomicUsize>) -> u16 {
    async fn accept_request(State(accepted): State<Arc<AtomicUsize>>) -> &'static str {
        accepted.fetch_add(1, Ordering::SeqCst);
        r#"{"id":"test","object":"chat.completion","created":0,"model":"test","choices":[]}"#
    }

    let key = KeyPair::generate().expect("self-signed key");
    let cert = CertificateParams::new(vec!["localhost".to_string()])
        .expect("certificate params")
        .self_signed(&key)
        .expect("self-signed certificate");

    let mut server_config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(
            vec![cert.der().clone()],
            rustls::pki_types::PrivateKeyDer::Pkcs8(key.serialize_der().into()),
        )
        .expect("server TLS config");
    server_config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    let app = Router::new()
        .route("/chat/completions", post(accept_request))
        .with_state(accepted);
    let listener = std::net::TcpListener::bind((bind, 0)).expect("bind server");
    listener.set_nonblocking(true).expect("set nonblocking");
    let port = listener.local_addr().expect("server address").port();
    let config = axum_server::tls_rustls::RustlsConfig::from_config(Arc::new(server_config));
    tokio::spawn(async move {
        axum_server::from_tcp_rustls(listener, config)
            .expect("create server")
            .serve(app.into_make_service())
            .await
            .expect("serve endpoint");
    });
    tokio::task::yield_now().await;
    port
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn self_signed_endpoint_is_reachable_only_through_the_allowlist() {
    support::pin_env();
    // Before the first client build: the allowlist latches at first read.
    xai_grok_extra_ca::set_insecure_tls_hosts(Some(vec!["127.0.0.1".to_string()]));
    assert!(xai_grok_extra_ca::insecure_tls_for_url("https://127.0.0.1:8443/v1"));
    assert!(!xai_grok_extra_ca::insecure_tls_for_url("https://127.0.0.2:8443/v1"));
    assert!(!xai_grok_extra_ca::insecure_tls_for_url("https://api.example.com/v1"));

    // Initialize the sampler's approved rustls provider before the test servers
    // build their own rustls config in this feature-unified test binary.
    drop(
        SamplingClient::new(test_config("https://127.0.0.1", "test-token"))
            .expect("initialize sampler TLS policy"),
    );

    let unlisted = Arc::new(AtomicUsize::new(0));
    let unlisted_port = start_self_signed_server("127.0.0.2", unlisted.clone()).await;
    let listed = Arc::new(AtomicUsize::new(0));
    let listed_port = start_self_signed_server("127.0.0.1", listed.clone()).await;

    let unlisted_url = format!("https://127.0.0.2:{unlisted_port}");
    let client = SamplingClient::new(test_config(&unlisted_url, "test-token")).expect("build client");
    let error = client
        .conversation(support::conversation_request())
        .await
        .expect_err("an unlisted host must fail certificate verification");
    let detail = format!("{error:?}").to_lowercase();
    assert!(
        detail.contains("certificate"),
        "expected a certificate-verification failure, got {detail}"
    );
    assert_eq!(
        unlisted.load(Ordering::SeqCst),
        0,
        "the request must never reach the unlisted endpoint"
    );

    let listed_url = format!("https://127.0.0.1:{listed_port}");
    send_one(&SamplingClient::new(test_config(&listed_url, "test-token")).expect("build client"))
        .await;
    assert_eq!(
        listed.load(Ordering::SeqCst),
        1,
        "the allowlisted IP must reach the endpoint"
    );
}

