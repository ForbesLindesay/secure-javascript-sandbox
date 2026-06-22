use std::collections::HashSet;
use std::fmt::{self, Debug, Display};
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::{Arc, Mutex};

use http_body_util::BodyExt;
use hyper::HeaderMap;
use hyper::header::{self, HeaderValue};
use hyper::{Method, Uri};
use serde::{Deserialize, Serialize};
use tokio::net::{TcpStream, lookup_host};
use tokio::time::timeout;
use wasmtime_wasi_http::io::TokioIo;
use wasmtime_wasi_http::p2::bindings::http::types::{DnsErrorPayload, ErrorCode};
use wasmtime_wasi_http::p2::hyper_request_error;
use wasmtime_wasi_http::p2::types::{IncomingResponse, OutgoingRequestConfig};

use crate::ip_utils::IpUtils;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestValidationOutcome {
    Allowed,
    Blocked,
}
impl Serialize for RequestValidationOutcome {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            RequestValidationOutcome::Allowed => serializer.serialize_str("ALLOWED"),
            RequestValidationOutcome::Blocked => serializer.serialize_str("BLOCKED"),
        }
    }
}

pub struct RequestHeaders<'a> {
    /// The request's method
    pub method: &'a Method,

    /// The request's URI
    pub uri: &'a Uri,

    /// The request's headers
    pub headers: &'a HeaderMap<HeaderValue>,
}

#[derive(Clone)]
pub(crate) struct Requests {
    inner: Arc<Mutex<Vec<(Uri, Option<SocketAddr>, RequestValidationOutcome)>>>,
}

impl Default for Requests {
    fn default() -> Self {
        Requests {
            inner: Default::default(),
        }
    }
}

impl Requests {
    pub fn push(&self, request: (Uri, Option<SocketAddr>, RequestValidationOutcome)) {
        self.inner.lock().unwrap().push(request)
    }
    pub fn take(&self) -> Vec<(Uri, Option<SocketAddr>, RequestValidationOutcome)> {
        std::mem::take(&mut *self.inner.lock().unwrap())
    }
}

pub trait CustomHttpMode: Clone + Send + Sync + 'static {
    fn can_send_request(&self, request: RequestHeaders) -> bool;
    fn can_connect(&self, address: SocketAddr) -> bool;
}

#[derive(Clone)]
pub(crate) struct BlockAllHttp;
impl CustomHttpMode for BlockAllHttp {
    fn can_send_request(&self, _request: RequestHeaders) -> bool {
        false
    }
    fn can_connect(&self, _address: SocketAddr) -> bool {
        false
    }
}

#[derive(Clone)]
pub enum HttpMode {
    AllowAll,
    AllowGlobalIpOnly,
    AllowListHosts(Arc<HashSet<String>>),
    BlockAll,
}

impl Default for HttpMode {
    fn default() -> Self {
        HttpMode::BlockAll
    }
}

#[derive(Deserialize)]
enum SerializedHttpMode {
    AllowAll,
    AllowGlobalIpOnly,
    AllowListHosts(Vec<String>),
    BlockAll,
}

impl<'de> Deserialize<'de> for HttpMode {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let helper = SerializedHttpMode::deserialize(deserializer)?;
        match helper {
            SerializedHttpMode::AllowAll => Ok(HttpMode::AllowAll),
            SerializedHttpMode::AllowGlobalIpOnly => Ok(HttpMode::AllowGlobalIpOnly),
            SerializedHttpMode::AllowListHosts(hosts) => {
                let host_set: HashSet<String> = hosts.into_iter().collect();
                Ok(HttpMode::AllowListHosts(Arc::new(host_set)))
            }
            SerializedHttpMode::BlockAll => Ok(HttpMode::BlockAll),
        }
    }
}

#[derive(Copy, Clone)]
pub struct InvalidHttpMode;
impl Display for InvalidHttpMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid HttpMode")
    }
}
impl Debug for InvalidHttpMode {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "invalid HttpMode")
    }
}

impl FromStr for HttpMode {
    type Err = InvalidHttpMode;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let result = match s {
            "ALLOW_ALL" => HttpMode::AllowAll,
            "ALLOW_GLOBAL_IP_ONLY" => HttpMode::AllowGlobalIpOnly,
            "BLOCK_ALL" => HttpMode::BlockAll,
            str if str.starts_with("ALLOW_LIST_HOSTS:") => {
                let hosts_str = &str["ALLOW_LIST_HOSTS:".len()..];
                let hosts: HashSet<String> = hosts_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                HttpMode::AllowListHosts(Arc::new(hosts))
            }
            _ => return Err(InvalidHttpMode),
        };
        Ok(result)
    }
}

impl CustomHttpMode for HttpMode {
    fn can_send_request(&self, request: RequestHeaders) -> bool {
        match self {
            HttpMode::AllowAll => true,
            HttpMode::AllowGlobalIpOnly => true,
            HttpMode::AllowListHosts(allowed_hosts) => {
                if let Some(host) = request.uri.host() {
                    allowed_hosts.contains(host)
                } else {
                    false
                }
            }
            HttpMode::BlockAll => false,
        }
    }
    fn can_connect(&self, address: SocketAddr) -> bool {
        match self {
            HttpMode::AllowAll => true,
            HttpMode::AllowGlobalIpOnly => address.ip().is_global_ext(),
            HttpMode::AllowListHosts(_) => true,
            HttpMode::BlockAll => false,
        }
    }
}

// Based on use wasmtime_wasi_http::types::default_send_request_handler;
// but extracted to allow hooking in our own logic for allowing/blocking requests
// and to handle redirects.
pub(crate) async fn send_request_handler(
    request: hyper::Request<wasmtime_wasi_http::p2::body::HyperOutgoingBody>,
    OutgoingRequestConfig {
        use_tls,
        connect_timeout,
        first_byte_timeout,
        between_bytes_timeout,
    }: wasmtime_wasi_http::p2::types::OutgoingRequestConfig,
    http_mode: &impl CustomHttpMode,
    requests: Requests,
) -> Result<wasmtime_wasi_http::p2::types::IncomingResponse, ErrorCode> {
    let mut redirect_count: u8 = 0;
    let mut next_request = Some(request);
    while let Some(request) = next_request {
        let (parts, body) = request.into_parts();
        let mut request = hyper::Request::from_parts(parts.clone(), body);
        if !http_mode.can_send_request(RequestHeaders {
            method: request.method(),
            uri: request.uri(),
            headers: request.headers(),
        }) {
            requests.push((
                request.uri().clone(),
                None,
                RequestValidationOutcome::Blocked,
            ));
            return Err(ErrorCode::DestinationNotFound);
        }

        let authority: String = if let Some(authority) = request.uri().authority() {
            if authority.port().is_some() {
                authority.to_string()
            } else {
                let port = if use_tls { 443 } else { 80 };
                format!("{}:{port}", authority.to_string())
            }
        } else {
            return Err(ErrorCode::HttpRequestUriInvalid);
        };

        if let Ok(value) = header::HeaderValue::from_str(&authority) {
            request.headers_mut().insert(header::HOST, value);
        } else {
            return Err(ErrorCode::HttpRequestUriInvalid);
        }

        let (tcp_stream, socket_addr) = timeout(
            connect_timeout,
            get_tcp_stream(request.uri(), &authority, http_mode, &requests),
        )
        .await
        .map_err(|_| ErrorCode::ConnectionTimeout)??;
        {
            requests.push((
                request.uri().clone(),
                Some(socket_addr),
                RequestValidationOutcome::Allowed,
            ));
        }
        let (mut sender, worker) = if use_tls {
            use rustls::pki_types::ServerName;

            // derived from https://github.com/rustls/rustls/blob/main/examples/src/bin/simpleclient.rs
            let root_cert_store = rustls::RootCertStore {
                roots: webpki_roots::TLS_SERVER_ROOTS.into(),
            };
            let config = rustls::ClientConfig::builder()
                .with_root_certificates(root_cert_store)
                .with_no_client_auth();
            let connector = tokio_rustls::TlsConnector::from(std::sync::Arc::new(config));
            let mut parts = authority.split(":");
            let host = parts.next().unwrap_or(&authority);
            let domain = ServerName::try_from(host)
                .map_err(|e| {
                    tracing::warn!("dns lookup error: {e:?}");
                    dns_error("invalid dns name".to_string(), 0)
                })?
                .to_owned();
            let stream = connector.connect(domain, tcp_stream).await.map_err(|e| {
                tracing::warn!("tls protocol error: {e:?}");
                ErrorCode::TlsProtocolError
            })?;
            let stream = TokioIo::new(stream);

            let (sender, conn) = timeout(
                connect_timeout,
                hyper::client::conn::http1::handshake(stream),
            )
            .await
            .map_err(|_| ErrorCode::ConnectionTimeout)?
            .map_err(hyper_request_error)?;

            let worker = wasmtime_wasi::runtime::spawn(async move {
                match conn.await {
                    Ok(()) => {}
                    // TODO: shouldn't throw away this error and ideally should
                    // surface somewhere.
                    Err(e) => tracing::warn!("dropping error {e}"),
                }
            });

            (sender, worker)
        } else {
            let tcp_stream = TokioIo::new(tcp_stream);
            let (sender, conn) = timeout(
                connect_timeout,
                // TODO: we should plumb the builder through the http context, and use it here
                hyper::client::conn::http1::handshake(tcp_stream),
            )
            .await
            .map_err(|_| ErrorCode::ConnectionTimeout)?
            .map_err(hyper_request_error)?;

            let worker = wasmtime_wasi::runtime::spawn(async move {
                match conn.await {
                    Ok(()) => {}
                    // TODO: same as above, shouldn't throw this error away.
                    Err(e) => tracing::warn!("dropping error {e}"),
                }
            });

            (sender, worker)
        };

        // at this point, the request contains the scheme and the authority, but
        // the http packet should only include those if addressing a proxy, so
        // remove them here, since SendRequest::send_request does not do it for us
        *request.uri_mut() = Uri::builder()
            .path_and_query(
                request
                    .uri()
                    .path_and_query()
                    .map(|p| p.as_str())
                    .unwrap_or("/"),
            )
            .build()
            .expect("comes from valid request");

        let resp = timeout(first_byte_timeout, sender.send_request(request))
            .await
            .map_err(|_| ErrorCode::ConnectionReadTimeout)?
            .map_err(hyper_request_error)?
            .map(|body| body.map_err(hyper_request_error).boxed_unsync());

        if is_redirect_status(resp.status()) {
            if redirect_count >= 20 {
                return Err(ErrorCode::LoopDetected);
            }
            let mut request = hyper::Request::from_parts(parts, Default::default());
            if request.method() != &Method::GET && request.method() != &Method::HEAD {
                *request.method_mut() = Method::GET;
                request.headers_mut().remove(header::CONTENT_ENCODING);
                request.headers_mut().remove(header::CONTENT_LANGUAGE);
                request.headers_mut().remove(header::CONTENT_LOCATION);
                request.headers_mut().remove(header::CONTENT_TYPE);
            }
            *request.uri_mut() = resp
                .headers()
                .get(header::LOCATION)
                .and_then(|location| location.to_str().ok())
                .and_then(|location_str| Uri::try_from(location_str).ok())
                .ok_or(ErrorCode::HttpRequestUriInvalid)?;
            next_request = Some(request);
            redirect_count += 1;
            continue;
        }
        return Ok(IncomingResponse {
            resp,
            worker: Some(worker),
            between_bytes_timeout,
        });
    }
    unreachable!()
}

async fn get_tcp_stream(
    uri: &Uri,
    authority: &str,
    http_mode: &impl CustomHttpMode,
    requests: &Requests,
) -> Result<(TcpStream, SocketAddr), ErrorCode> {
    let hosts = lookup_host(&authority)
        .await
        .map_err(|_| dns_error("address not available".to_string(), 0))?;

    let mut last_err = None;
    for addr in hosts {
        if !http_mode.can_connect(addr) {
            requests.push((uri.clone(), Some(addr), RequestValidationOutcome::Blocked));
            return Err(ErrorCode::DestinationIpProhibited);
        }
        let tcp_stream = TcpStream::connect(addr).await;
        match tcp_stream {
            Ok(stream) => return Ok((stream, addr)),
            Err(err) => {
                last_err = Some(err);
            }
        }
    }

    return Err(last_err
        .map(|e| match e.kind() {
            std::io::ErrorKind::AddrNotAvailable => {
                dns_error("address not available".to_string(), 0)
            }

            _ => {
                if e.to_string()
                    .starts_with("failed to lookup address information")
                {
                    dns_error("address not available".to_string(), 0)
                } else {
                    ErrorCode::ConnectionRefused
                }
            }
        })
        .unwrap_or_else(|| dns_error("address not available".to_string(), 0)));
}

fn is_redirect_status(status: hyper::StatusCode) -> bool {
    matches!(
        status,
        hyper::StatusCode::MOVED_PERMANENTLY
            | hyper::StatusCode::FOUND
            | hyper::StatusCode::SEE_OTHER
            | hyper::StatusCode::TEMPORARY_REDIRECT
            | hyper::StatusCode::PERMANENT_REDIRECT
    )
}

fn dns_error(rcode: String, info_code: u16) -> ErrorCode {
    ErrorCode::DnsError(DnsErrorPayload {
        rcode: Some(rcode),
        info_code: Some(info_code),
    })
}
