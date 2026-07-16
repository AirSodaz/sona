use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::State;
use axum::http::{HeaderMap, Request, Response, StatusCode};
use axum_server::tls_rustls::RustlsConfig;
use rcgen::generate_simple_self_signed;
use sona_core::sync::{
    SyncDeleteResult, SyncObjectKey, SyncObjectPrefix, SyncObjectStore, SyncPutResult,
};

use super::*;

#[derive(Clone, Debug)]
struct LoggedRequest {
    method: String,
    path: String,
    if_match: Option<String>,
    if_none_match: Option<String>,
}

#[derive(Default)]
struct ServerData {
    collections: BTreeSet<String>,
    objects: BTreeMap<String, (Vec<u8>, String)>,
    requests: Vec<LoggedRequest>,
    next_etag: u64,
    redirect_location: Option<String>,
    omit_get_etag: bool,
    omit_put_etag: bool,
}

#[derive(Clone, Default)]
struct ServerState(Arc<Mutex<ServerData>>);

struct TestServer {
    base_url: String,
    state: ServerState,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for TestServer {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl TestServer {
    async fn start() -> Self {
        let certified = generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let tls = RustlsConfig::from_pem(
            certified.cert.pem().into_bytes(),
            certified.key_pair.serialize_pem().into_bytes(),
        )
        .await
        .unwrap();
        let listener = std::net::TcpListener::bind(("127.0.0.1", 0)).unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        let state = ServerState::default();
        state
            .0
            .lock()
            .unwrap()
            .collections
            .insert("/dav/".to_string());
        let app = Router::new()
            .fallback(webdav_handler)
            .with_state(state.clone());
        let task = tokio::spawn(async move {
            axum_server::from_tcp_rustls(listener, tls)
                .serve(app.into_make_service())
                .await
                .unwrap();
        });
        Self {
            base_url: format!("https://localhost:{}/dav/", address.port()),
            state,
            task,
        }
    }

    fn store(&self, timeout: Duration) -> WebDavObjectStore {
        let config =
            WebDavObjectStoreConfig::new(&self.base_url, "root", "sync-user", "sync-password")
                .unwrap();
        let server_url = parse_server_url(&config.server_url).unwrap();
        let root_url = build_collection_url(server_url.as_str(), &config.remote_root).unwrap();
        let client = Client::builder()
            .danger_accept_invalid_certs(true)
            .https_only(true)
            .timeout(timeout)
            .redirect(webdav_redirect_policy(server_url.clone(), root_url.clone()))
            .build()
            .unwrap();
        WebDavObjectStore {
            config,
            client,
            server_url,
            root_url,
        }
    }
}

async fn webdav_handler(
    State(state): State<ServerState>,
    request: Request<Body>,
) -> Response<Body> {
    let method = request.method().as_str().to_string();
    let path = request.uri().path().to_string();
    let headers = request.headers().clone();
    let body = to_bytes(request.into_body(), 80 * 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    let mut data = state.0.lock().unwrap();
    data.requests.push(LoggedRequest {
        method: method.clone(),
        path: path.clone(),
        if_match: header(&headers, "if-match"),
        if_none_match: header(&headers, "if-none-match"),
    });

    if path.ends_with("redirect.sync") {
        let location = data.redirect_location.clone().unwrap();
        return response(
            StatusCode::FOUND,
            &[],
            Body::empty(),
            Some(("location", location)),
        );
    }

    match method.as_str() {
        "PROPFIND" => propfind_response(&data, &path, header(&headers, "depth").as_deref()),
        "MKCOL" => {
            data.collections.insert(collection_path(&path));
            response(StatusCode::CREATED, &[], Body::empty(), None)
        }
        "GET" | "HEAD" => match data.objects.get(&path) {
            Some((bytes, etag)) => {
                let headers = if data.omit_get_etag {
                    Vec::new()
                } else {
                    vec![("etag", etag.as_str())]
                };
                response(
                    StatusCode::OK,
                    &headers,
                    if method == "HEAD" {
                        Body::empty()
                    } else {
                        Body::from(bytes.clone())
                    },
                    None,
                )
            }
            None => response(StatusCode::NOT_FOUND, &[], Body::empty(), None),
        },
        "PUT" => {
            let existing_etag = data.objects.get(&path).map(|(_, etag)| etag.clone());
            if header(&headers, "if-none-match").as_deref() == Some("*") && existing_etag.is_some()
            {
                return response(
                    StatusCode::PRECONDITION_FAILED,
                    &[("etag", existing_etag.as_deref().unwrap())],
                    Body::empty(),
                    None,
                );
            }
            if let Some(expected) = header(&headers, "if-match")
                && existing_etag.as_deref() != Some(expected.as_str())
            {
                return response(StatusCode::PRECONDITION_FAILED, &[], Body::empty(), None);
            }
            data.next_etag += 1;
            let etag = format!("\"etag-{}\"", data.next_etag);
            data.objects.insert(path.clone(), (body, etag.clone()));
            let ambiguous = path.ends_with("ambiguous.sync");
            let omit_put_etag = data.omit_put_etag;
            drop(data);
            if ambiguous {
                std::thread::sleep(Duration::from_millis(250));
            }
            let headers = if omit_put_etag {
                Vec::new()
            } else {
                vec![("etag", etag.as_str())]
            };
            response(StatusCode::CREATED, &headers, Body::empty(), None)
        }
        "DELETE" => {
            let Some((_, etag)) = data.objects.get(&path) else {
                return response(StatusCode::NOT_FOUND, &[], Body::empty(), None);
            };
            if let Some(expected) = header(&headers, "if-match")
                && expected != *etag
            {
                return response(
                    StatusCode::PRECONDITION_FAILED,
                    &[("etag", etag.as_str())],
                    Body::empty(),
                    None,
                );
            }
            data.objects.remove(&path);
            response(StatusCode::NO_CONTENT, &[], Body::empty(), None)
        }
        _ => response(StatusCode::METHOD_NOT_ALLOWED, &[], Body::empty(), None),
    }
}

fn propfind_response(data: &ServerData, path: &str, depth: Option<&str>) -> Response<Body> {
    let path = collection_path(path);
    if !data.collections.contains(&path) {
        return response(StatusCode::NOT_FOUND, &[], Body::empty(), None);
    }
    let mut entries = vec![collection_xml(&path)];
    if depth == Some("1") {
        entries.extend(
            data.collections
                .iter()
                .filter(|candidate| *candidate != &path && direct_child(&path, candidate))
                .map(|candidate| collection_xml(candidate)),
        );
        entries.extend(
            data.objects
                .iter()
                .filter(|(candidate, _)| direct_child(&path, candidate))
                .map(|(candidate, (bytes, etag))| object_xml(candidate, bytes.len(), etag)),
        );
    }
    let xml = format!(
        "<?xml version=\"1.0\"?><d:multistatus xmlns:d=\"DAV:\">{}</d:multistatus>",
        entries.join("")
    );
    response(
        StatusCode::MULTI_STATUS,
        &[("content-type", "application/xml")],
        Body::from(xml),
        None,
    )
}

fn direct_child(parent: &str, candidate: &str) -> bool {
    let Some(remainder) = candidate.strip_prefix(parent) else {
        return false;
    };
    !remainder.is_empty() && !remainder.trim_end_matches('/').contains('/')
}

fn collection_path(path: &str) -> String {
    if path.ends_with('/') {
        path.to_string()
    } else {
        format!("{path}/")
    }
}

fn collection_xml(path: &str) -> String {
    format!(
        "<d:response><d:href>{path}</d:href><d:propstat><d:prop><d:resourcetype><d:collection /></d:resourcetype></d:prop></d:propstat></d:response>"
    )
}

fn object_xml(path: &str, size: usize, etag: &str) -> String {
    format!(
        "<d:response><d:href>{path}</d:href><d:propstat><d:prop><d:getetag>{etag}</d:getetag><d:getcontentlength>{size}</d:getcontentlength><d:resourcetype /></d:prop></d:propstat></d:response>"
    )
}

fn header(headers: &HeaderMap, name: &str) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(ToOwned::to_owned)
}

fn response(
    status: StatusCode,
    headers: &[(&str, &str)],
    body: Body,
    extra: Option<(&str, String)>,
) -> Response<Body> {
    let mut builder = Response::builder().status(status);
    for (name, value) in headers {
        builder = builder.header(*name, *value);
    }
    if let Some((name, value)) = extra {
        builder = builder.header(name, value);
    }
    builder.body(body).unwrap()
}

#[tokio::test]
async fn object_store_contract_maps_webdav_methods_and_conditions() {
    let server = TestServer::start().await;
    let store = server.store(Duration::from_secs(2));
    let capabilities = store.probe().await.unwrap();
    assert!(capabilities.conditional_create);
    assert!(capabilities.compare_and_swap);
    assert!(capabilities.delete);

    let key = SyncObjectKey::parse("sona-sync/v1/vault-a/vault.json").unwrap();
    let created = store.put_if_absent(&key, b"first".to_vec()).await.unwrap();
    let first_etag = match created {
        SyncPutResult::Created { etag: Some(etag) } => etag,
        other => panic!("unexpected create result: {other:?}"),
    };
    assert!(matches!(
        store
            .put_if_absent(&key, b"ignored".to_vec())
            .await
            .unwrap(),
        SyncPutResult::AlreadyExists { .. }
    ));
    assert_eq!(store.get(&key).await.unwrap().unwrap().bytes, b"first");
    assert!(matches!(
        store
            .compare_and_swap(&key, Some("\"wrong\""), b"second".to_vec())
            .await
            .unwrap(),
        SyncPutResult::Conflict { .. }
    ));
    let second_etag = match store
        .compare_and_swap(&key, Some(&first_etag), b"second".to_vec())
        .await
        .unwrap()
    {
        SyncPutResult::Created { etag: Some(etag) } => etag,
        other => panic!("unexpected CAS result: {other:?}"),
    };

    let page = store
        .list(&SyncObjectPrefix::parse("sona-sync/v1").unwrap(), None)
        .await
        .unwrap();
    assert_eq!(page.objects.len(), 1);
    assert_eq!(page.objects[0].key, key);
    assert!(matches!(
        store.delete(&key, Some("\"wrong\"")).await.unwrap(),
        SyncDeleteResult::Conflict { .. }
    ));
    assert_eq!(
        store.delete(&key, Some(&second_etag)).await.unwrap(),
        SyncDeleteResult::Deleted
    );

    let requests = server.state.0.lock().unwrap().requests.clone();
    for method in ["PROPFIND", "MKCOL", "GET", "PUT", "DELETE"] {
        assert!(requests.iter().any(|request| request.method == method));
    }
    assert!(requests.iter().any(|request| {
        request.method == "PUT" && request.if_none_match.as_deref() == Some("*")
    }));
    assert!(requests.iter().any(|request| {
        request.method == "PUT" && request.if_match.as_deref() == Some(first_etag.as_str())
    }));
}

#[tokio::test]
async fn probe_recovers_etags_from_propfind_when_object_responses_omit_them() {
    let server = TestServer::start().await;
    {
        let mut data = server.state.0.lock().unwrap();
        data.omit_get_etag = true;
        data.omit_put_etag = true;
    }
    let store = server.store(Duration::from_secs(2));

    let capabilities = store.probe().await.unwrap();

    assert!(capabilities.conditional_create);
    assert!(capabilities.compare_and_swap);
    assert!(capabilities.delete);
    let requests = server.state.0.lock().unwrap().requests.clone();
    assert!(requests.iter().any(|request| {
        request.method == "PROPFIND" && request.path.contains(".sona-sync-probe")
    }));
}

#[tokio::test]
async fn ambiguous_put_timeout_is_recovered_by_reading_identical_bytes() {
    let server = TestServer::start().await;
    let store = server.store(Duration::from_millis(100));
    let key = SyncObjectKey::parse("sona-sync/v1/vault-a/ambiguous.sync").unwrap();

    let result = store
        .put_if_absent(&key, b"ciphertext".to_vec())
        .await
        .unwrap();

    assert!(matches!(result, SyncPutResult::AlreadyExists { .. }));
    assert_eq!(store.get(&key).await.unwrap().unwrap().bytes, b"ciphertext");
}

#[tokio::test]
async fn redirect_outside_the_configured_root_is_rejected_before_following() {
    let server = TestServer::start().await;
    server.state.0.lock().unwrap().redirect_location =
        Some(format!("{}outside/stolen", server.base_url));
    let store = server.store(Duration::from_secs(2));
    let key = SyncObjectKey::parse("redirect.sync").unwrap();

    let error = store.get(&key).await.unwrap_err();

    assert!(error.to_string().contains("redirect"));
    let requests = server.state.0.lock().unwrap().requests.clone();
    assert!(
        !requests
            .iter()
            .any(|request| request.path.contains("outside"))
    );
}
