//! Shared HTTP proxy selection and connection pools for Sunrise downloads.
//!
//! Local hosts and hosts in `NO_PROXY` go direct. Other URLs use the launcher setting when present,
//! then `HTTPS_PROXY` or `HTTP_PROXY` for the URL's scheme, then `ALL_PROXY`.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fmt;
use std::net::Ipv6Addr;
use std::num::NonZeroU16;
use std::sync::{Mutex, MutexGuard};

/// What the launcher set, when it did.
static PROXY: Mutex<Option<String>> = Mutex::new(None);

/// Agents kept for reuse, named by the part that asked and the proxy they use: a fresh `Agent`
/// starts a new connection pool, so building one per request costs a handshake per file.
static KEPT: Mutex<Option<HashMap<AgentKey, ureq::Agent>>> = Mutex::new(None);

#[derive(PartialEq, Eq, Hash)]
struct AgentKey {
    slot: String,
    proxy: Option<String>,
}

/// A configuration error that can be reported without showing the proxy address or credentials.
#[derive(Debug)]
pub struct ProxyError {
    setting: &'static str,
    kind: InvalidProxy,
}

#[derive(Debug, PartialEq, Eq)]
enum InvalidProxy {
    Authority,
    Scheme,
    Encoding,
}

impl fmt::Display for ProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self.kind {
            InvalidProxy::Authority => "invalid proxy authority",
            InvalidProxy::Scheme => "unsupported proxy scheme",
            InvalidProxy::Encoding => "proxy value is not valid Unicode",
        };
        write!(f, "{}: {reason}", self.setting)
    }
}

impl std::error::Error for ProxyError {}

/// The proxy to use from now on: the launcher's setting, or None to fall back to the environment.
/// The agents built so far are dropped when it changes, since they hold the address it replaces.
pub fn set_proxy(spec: Option<String>) {
    let spec = spec.map(|s| s.trim().to_owned()).filter(|s| !s.is_empty());
    let mut proxy = lock(&PROXY);
    if *proxy == spec {
        return;
    }
    *proxy = spec;
    drop(proxy);
    *lock(&KEPT) = None;
}

/// A kept agent for `url`, using the launcher setting or the environment unless the destination
/// is reached directly. An invalid selected proxy is an error, not a direct route.
/// Each `slot` names one builder configuration. `configure` only runs when the slot has no agent
/// for the selected proxy, and should not set a proxy itself.
pub fn agent(
    slot: &str,
    url: &str,
    configure: impl FnOnce(ureq::AgentBuilder) -> ureq::AgentBuilder,
) -> Result<ureq::Agent, ProxyError> {
    let destination = url::Url::parse(url).ok();
    let selected = if destination
        .as_ref()
        .is_some_and(|url| bypasses_proxy(url, &no_proxy()))
    {
        None
    } else if let Some(spec) = lock(&PROXY).clone() {
        Some(("launcher proxy setting", spec))
    } else {
        let scheme = destination.as_ref().map_or("https", url::Url::scheme);
        env_proxy(scheme, |name| std::env::var_os(name))?
    };
    keep(
        slot,
        selected.as_ref().map(|(_, spec)| spec.as_str()),
        |proxy| {
            let builder = configure(ureq::AgentBuilder::new()).try_proxy_from_env(false);
            match proxy {
                Some(proxy) => builder.proxy(proxy).build(),
                None => builder.build(),
            }
        },
    )
    .map_err(|error| ProxyError {
        setting: selected.map_or(error.setting, |(name, _)| name),
        ..error
    })
}

/// An agent built by `build` and kept for the next request with the same `slot` and proxy.
/// Building a fresh agent every time would give each request a new connection pool.
/// Only `None` requests a direct agent; an invalid `spec` returns an error without calling `build`.
pub fn keep(
    slot: &str,
    spec: Option<&str>,
    build: impl FnOnce(Option<ureq::Proxy>) -> ureq::Agent,
) -> Result<ureq::Agent, ProxyError> {
    let key = AgentKey {
        slot: slot.to_owned(),
        proxy: spec.map(str::to_owned),
    };
    if let Some(agent) = lock(&KEPT).as_ref().and_then(|kept| kept.get(&key)) {
        return Ok(agent.clone());
    }
    let proxy = spec
        .map(parse_proxy)
        .transpose()
        .map_err(|kind| ProxyError {
            setting: "proxy setting",
            kind,
        })?;
    let agent = build(proxy);
    lock(&KEPT)
        .get_or_insert_with(HashMap::new)
        .insert(key, agent.clone());
    Ok(agent)
}

/// `spec` as a proxy, or `None` when the address cannot be used. For a download, use [`agent`] or
/// [`keep`] so an invalid proxy is reported instead of being confused with an unset proxy.
pub fn parse(spec: &str) -> Option<ureq::Proxy> {
    parse_proxy(spec).ok()
}

fn parse_proxy(spec: &str) -> Result<ureq::Proxy, InvalidProxy> {
    let spec = spec.trim_end_matches('/');
    let (scheme, authority) = spec.split_once("://").unwrap_or(("http", spec));
    let scheme = match scheme {
        "http" | "socks4" | "socks4a" | "socks" | "socks5" => scheme,
        // ureq's SOCKS5 already sends the proxy host names to resolve.
        "socks5h" => "socks5",
        scheme if valid_scheme(scheme) => return Err(InvalidProxy::Scheme),
        _ => return Err(InvalidProxy::Authority),
    };
    // ureq splits the address at colons and silently defaults an unparseable port. Reject a
    // malformed authority (including IPv6 proxy endpoints, which that parser cannot represent).
    let address = authority
        .rsplit_once('@')
        .map_or(authority, |(_, addr)| addr);
    let (host, port) = address
        .split_once(':')
        .map_or((address, None), |(host, port)| (host, Some(port)));
    if host.is_empty()
        || host
            .chars()
            .any(|c| c.is_whitespace() || "[]/\\?#".contains(c))
        || authority.chars().any(char::is_control)
        || port.is_some_and(|p| {
            !p.bytes().all(|b| b.is_ascii_digit()) || p.parse::<NonZeroU16>().is_err()
        })
    {
        return Err(InvalidProxy::Authority);
    }
    ureq::Proxy::new(format!("{scheme}://{authority}")).map_err(|_| InvalidProxy::Authority)
}

/// `spec` as it may be shown: a user name and password in it become `***`. Everything up to the
/// last `@`, excluding a valid scheme prefix, is hidden so unescaped delimiters in passwords aren't
/// shown either. Diagnostics for invalid settings should not display their value at all.
pub fn redacted(spec: &str) -> String {
    let prefix_len = spec
        .split_once("://")
        .filter(|(scheme, _)| valid_scheme(scheme))
        .map_or(0, |(scheme, _)| scheme.len() + 3);
    match spec.rfind('@') {
        Some(at) => format!("{}***@{}", &spec[..prefix_len], &spec[at + 1..]),
        None => spec.to_owned(),
    }
}

fn valid_scheme(scheme: &str) -> bool {
    matches!(scheme.as_bytes(), [first, rest @ ..] if first.is_ascii_alphabetic()
        && rest.iter().all(|b| b.is_ascii_alphanumeric() || b"+-.".contains(b)))
}

/// Whether `url` is reached without the proxy: a local address, or a host `NO_PROXY` names. The
/// launcher asks this of its own requests, so both reach the same hosts directly.
pub fn direct(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| bypasses_proxy(&url, &no_proxy()))
}

/// `HTTPS_PROXY` (`HTTP_PROXY` for an http address), then `ALL_PROXY`, in the upper-case spelling
/// first and the lower-case one after, as the tools that write them do. For routing a request,
/// use [`agent`] to distinguish a missing setting from one that cannot be decoded.
pub fn env_for(scheme: &str) -> Option<String> {
    env_proxy(scheme, |name| std::env::var_os(name))
        .ok()
        .flatten()
        .map(|(_, spec)| spec)
}

fn env_proxy(
    scheme: &str,
    mut read: impl FnMut(&str) -> Option<OsString>,
) -> Result<Option<(&'static str, String)>, ProxyError> {
    let names = if scheme.eq_ignore_ascii_case("http") {
        ["HTTP_PROXY", "http_proxy", "ALL_PROXY", "all_proxy"]
    } else {
        ["HTTPS_PROXY", "https_proxy", "ALL_PROXY", "all_proxy"]
    };
    names
        .into_iter()
        .find_map(|name| {
            let value = read(name)?;
            let Some(value) = value.to_str() else {
                return Some(Err(ProxyError {
                    setting: name,
                    kind: InvalidProxy::Encoding,
                }));
            };
            let value = value.trim();
            (!value.is_empty()).then(|| Ok((name, value.to_owned())))
        })
        .transpose()
}

/// `NO_PROXY`'s list: the hosts reached directly whatever the proxy is.
fn no_proxy() -> String {
    ["NO_PROXY", "no_proxy"]
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .unwrap_or_default()
}

/// The host part of a URL (`https://user@host:port/path` is `host`).
pub fn host_of(url: &str) -> Option<&str> {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let rest = rest.split(['/', '?', '#']).next()?;
    let rest = rest.rsplit_once('@').map_or(rest, |(_, host)| host);
    Some(match rest.rfind(':') {
        // A port follows the last colon; an IPv6 literal keeps the colons inside its brackets.
        Some(at) if !rest[at..].contains(']') => &rest[..at],
        _ => rest,
    })
}

/// `https` in `https://…`, or None for anything that isn't an absolute address.
pub fn scheme_of(url: &str) -> Option<&str> {
    url.split_once("://").map(|(scheme, _)| scheme)
}

/// Whether `host` is this computer: the game's own API and a server on the same machine.
fn local(host: &str) -> bool {
    let host = bare(host);
    host == "localhost"
        || host.ends_with(".localhost")
        || host == "::1"
        || host == "0.0.0.0"
        || is_loopback_v4(&host)
}

/// A dotted IPv4 address in `127.0.0.0/8`; a DNS name that merely starts with `127.` is not one.
fn is_loopback_v4(host: &str) -> bool {
    host.starts_with("127.")
        && host
            .split('.')
            .all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit()))
}

/// Whether the destination is local or in `NO_PROXY`: `*`, `host`, `.domain`, `*.domain` and
/// `host:port` entries (matching the effective port, including the scheme's default).
fn bypasses_proxy(url: &url::Url, no_proxy: &str) -> bool {
    let Some(host) = url.host_str() else {
        return false;
    };
    if local(host) {
        return true;
    }
    let host = bare(host);
    let port = url.port_or_known_default();
    no_proxy
        .split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
        .any(|entry| {
            if entry == "*" {
                return true;
            }
            let entry = entry.trim_start_matches('*').trim_start_matches('.');
            // A port follows the last colon of a bracketed entry, or of a host with no colon
            // before it; an unbracketed IPv6 literal keeps its colons.
            let entry = match entry.rfind(':') {
                Some(at)
                    if !entry[at..].contains(']')
                        && (entry[..at].ends_with(']') || !entry[..at].contains(':')) =>
                {
                    if !entry[at + 1..]
                        .parse::<u16>()
                        .is_ok_and(|required| Some(required) == port)
                    {
                        return false;
                    }
                    &entry[..at]
                }
                _ => entry,
            };
            let entry = bare(entry);
            !entry.is_empty() && (host == entry || host.ends_with(&format!(".{entry}")))
        })
}

/// A lower-case host name or canonical IP literal, without IPv6 brackets.
fn bare(host: &str) -> String {
    let host = host.trim().trim_matches(['[', ']']);
    if host.contains(':')
        && let Ok(ip) = host.parse::<Ipv6Addr>()
    {
        return ip.to_string();
    }
    host.to_ascii_lowercase()
}

/// The lock, even after a thread panicked while holding it: an address lost to a panic is worse
/// than an old one kept.
fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the tests below share, since the address and the agents are one of each for the whole
    /// process: they take it in turn.
    fn alone() -> MutexGuard<'static, ()> {
        static TURN: Mutex<()> = Mutex::new(());
        lock(&TURN)
    }

    #[test]
    fn credentials_are_not_shown() {
        assert_eq!(redacted("http://127.0.0.1:7890"), "http://127.0.0.1:7890");
        assert_eq!(redacted("proxy.local:3128"), "proxy.local:3128");
        assert_eq!(
            redacted("http://user:pass@proxy.local:3128"),
            "http://***@proxy.local:3128"
        );
        assert_eq!(
            redacted("socks5://u:p@10.0.0.1:1080"),
            "socks5://***@10.0.0.1:1080"
        );
        assert_eq!(
            redacted("user:pass@proxy.local:3128"),
            "***@proxy.local:3128"
        );
        assert_eq!(redacted("http://u:p/a@ss@proxy:1"), "http://***@proxy:1");
        assert_eq!(redacted("http://user:pass@"), "http://***@");
        assert_eq!(
            redacted("dummy-user:dummy-password://with@delimiters@proxy.local:3128"),
            "***@proxy.local:3128"
        );
    }

    #[test]
    fn a_proxy_the_launcher_sets_is_trimmed_and_kept() {
        let _turn = alone();
        set_proxy(Some("  http://127.0.0.1:7890  ".into()));
        assert_eq!(
            PROXY.lock().unwrap().as_deref(),
            Some("http://127.0.0.1:7890")
        );
        set_proxy(Some("   ".into()));
        assert!(PROXY.lock().unwrap().is_none());
    }

    /// An address on this computer is reached directly whatever `NO_PROXY` says: what an import
    /// needs when the launcher's own requests to it go direct.
    #[test]
    fn a_local_address_is_reached_directly() {
        assert!(direct("http://127.0.0.1:8130/api/v1/status"));
        assert!(direct("http://localhost/api/v1/status"));
        assert!(direct("https://[::1]:25565/"));
    }

    #[test]
    fn a_host_is_read_out_of_the_address() {
        assert_eq!(
            host_of("https://cdn.modrinth.com/v2/x.jar"),
            Some("cdn.modrinth.com")
        );
        assert_eq!(
            host_of("https://user:pass@piston-meta.mojang.com:8443/a"),
            Some("piston-meta.mojang.com")
        );
        assert_eq!(
            host_of("http://127.0.0.1:8130/api/v1/status"),
            Some("127.0.0.1")
        );
        assert_eq!(host_of("https://[::1]:25565/"), Some("[::1]"));
        assert_eq!(host_of("https://example.com?x=1"), Some("example.com"));
        assert_eq!(host_of("https://example.com#top"), Some("example.com"));
    }

    #[test]
    fn only_absolute_addresses_have_a_scheme() {
        assert_eq!(scheme_of("https://a.b"), Some("https"));
        assert_eq!(scheme_of("http://a.b"), Some("http"));
        assert_eq!(scheme_of("a.b/c"), None);
    }

    #[test]
    fn this_computer_is_never_proxied() {
        for host in [
            "localhost",
            "LOCALHOST",
            "[::1]",
            "::1",
            "127.0.0.1",
            "127.1.2.3",
        ] {
            assert!(local(host), "{host}");
        }
        for host in [
            "playsunrise.net",
            "api.minecraftservices.com",
            "10.0.0.1",
            "127.example.com",
            "127.0.0.1.example.com",
        ] {
            assert!(!local(host), "{host}");
        }
    }

    fn bypasses(url: &str, no_proxy: &str) -> bool {
        bypasses_proxy(&url::Url::parse(url).unwrap(), no_proxy)
    }

    /// What an import with `NO_PROXY=modrinth.com` needs: the launcher's own requests to it go
    /// direct, so the jars fetched for it must too.
    #[test]
    fn no_proxy_lists_hosts_and_their_subdomains() {
        assert!(bypasses("https://anything.at.all", "*"));
        assert!(bypasses("https://cdn.modrinth.com", "modrinth.com"));
        assert!(bypasses("https://api.modrinth.com", "MODRINTH.com"));
        assert!(bypasses("https://piston-meta.mojang.com", "*.mojang.com"));
        assert!(bypasses(
            "https://wiki.corp.example",
            " .corp.example , other.host "
        ));
        assert!(!bypasses("https://notmodrinth.com", "modrinth.com"));
        assert!(!bypasses(
            "https://modrinth.com",
            " .corp.example , mojang.com "
        ));
        assert!(!bypasses("https://edge.forgecdn.net", "modrinth.com"));
        assert!(!bypasses("https://modrinth.com", ""));
    }

    #[test]
    fn no_proxy_requires_the_specified_destination_port() {
        assert!(bypasses(
            "https://mirror.example:8443/",
            "mirror.example:8443"
        ));
        assert!(!bypasses("https://mirror.example/", "mirror.example:8443"));
        assert!(bypasses("https://mirror.example/", "mirror.example:443"));
        assert!(!bypasses("http://mirror.example/", "mirror.example:443"));
        assert!(bypasses("http://mirror.example/", "mirror.example:80"));
        assert!(!bypasses("http://mirror.example/", "mirror.example:http"));
        assert!(bypasses("https://mirror.example:8443/", "mirror.example"));
        assert!(!bypasses(
            r"https://other.example\@mirror.example:8443/",
            "mirror.example:8443"
        ));
    }

    #[test]
    fn no_proxy_matches_ipv6_literals_with_or_without_brackets() {
        let url = "http://[2001:0db8::1]:8080/";
        assert!(bypasses(url, "2001:db8::1"));
        assert!(bypasses(url, "[2001:db8::1]"));
        assert!(bypasses(url, "[2001:0db8::1]:8080"));
        assert!(!bypasses(url, "[2001:db8::1]:8081"));
        assert!(!bypasses(url, "2001:db8::2"));
    }

    #[test]
    fn a_selected_invalid_proxy_never_builds_a_direct_agent() {
        let _turn = alone();
        // An existing direct agent must not be reused when selection produces an invalid proxy.
        keep("invalid", None, |_| ureq::AgentBuilder::new().build()).unwrap();
        for (spec, kind) in [
            (
                "http://proxy.example:dummy-password",
                InvalidProxy::Authority,
            ),
            ("http://proxy.example:65536", InvalidProxy::Authority),
            ("http://proxy.example:", InvalidProxy::Authority),
            ("http://:3128", InvalidProxy::Authority),
            ("http://proxy.example/path", InvalidProxy::Authority),
            (
                "dummy-user:dummy-password://with@delimiters@proxy.example",
                InvalidProxy::Authority,
            ),
            (
                "https://dummy-user:dummy-password@proxy.example",
                InvalidProxy::Scheme,
            ),
        ] {
            let fail = |_| panic!("an invalid proxy must not be used to build an agent");
            let error = keep("invalid", Some(spec), fail).unwrap_err();
            assert_eq!(error.kind, kind);
            let message = error.to_string();
            assert!(!message.contains("dummy-user"));
            assert!(!message.contains("dummy-password"));
        }
        for spec in [
            "http://127.0.0.1:7890",
            "127.0.0.1:7890",
            "dummy-user:dummy-password@proxy.example:3128",
            "socks5://127.0.0.1:7891",
        ] {
            assert!(parse(spec).is_some());
        }
    }

    /// A stand-in SOCKS5 proxy: answers one request itself and returns the host it was asked for.
    fn socks5_proxy() -> (u16, std::thread::JoinHandle<String>) {
        use std::io::{BufRead, BufReader, Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut greeting = [0; 2];
            stream.read_exact(&mut greeting).unwrap();
            assert_eq!(greeting[0], 5);
            let mut methods = vec![0; greeting[1].into()];
            stream.read_exact(&mut methods).unwrap();
            stream.write_all(&[5, 0]).unwrap();
            let mut request = [0; 5];
            stream.read_exact(&mut request).unwrap();
            assert_eq!(request[..4], [5, 1, 0, 3], "a CONNECT to a host name");
            let mut host = vec![0; request[4].into()];
            stream.read_exact(&mut host).unwrap();
            let mut target_port = [0; 2];
            stream.read_exact(&mut target_port).unwrap();
            stream.write_all(&[5, 0, 0, 1, 0, 0, 0, 0, 0, 0]).unwrap();
            let mut http = BufReader::new(stream.try_clone().unwrap());
            let mut line = String::new();
            while http.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok")
                .unwrap();
            String::from_utf8(host).unwrap()
        });
        (port, server)
    }

    /// `.invalid` never resolves here (RFC 2606), so the answer can only have come through the
    /// proxy, which is handed the name to resolve.
    #[test]
    #[ignore = "daily"]
    fn a_request_goes_through_a_socks_proxy() {
        for scheme in ["socks5", "socks5h"] {
            let (port, server) = socks5_proxy();
            let proxy = parse(&format!("{scheme}://127.0.0.1:{port}")).unwrap();
            let agent = ureq::AgentBuilder::new().proxy(proxy).build();
            let answer = agent.get("http://sunrise.invalid/a.jar").call().unwrap();
            assert_eq!(answer.into_string().unwrap(), "ok", "{scheme}");
            assert_eq!(server.join().unwrap(), "sunrise.invalid", "{scheme}");
        }
    }

    #[test]
    #[ignore = "daily"]
    fn a_configured_agent_uses_the_env_proxy_but_goes_direct_for_localhost() {
        use std::io::{BufRead, BufReader, Write};
        use std::net::TcpListener;
        use std::time::Duration;

        const LOCAL_URL: &str = "SUNRISE_PROXY_TEST_LOCAL_URL";
        const TIMEOUT: Duration = Duration::from_secs(5);
        if let Ok(local_url) = std::env::var(LOCAL_URL) {
            let get = |url: &str| {
                agent("test-configured", url, |builder| {
                    builder.redirects(0).timeout(TIMEOUT)
                })
                .unwrap()
                .get(url)
                .call()
                .unwrap()
            };
            assert_eq!(
                get("http://sunrise.invalid/a.jar").into_string().unwrap(),
                "ok"
            );
            assert_eq!(
                get(&local_url).status(),
                302,
                "the direct agent must not redirect"
            );
            let error = agent("test-configured", "https://sunrise.invalid/", |_| {
                panic!("a rejected setting must stop before building the agent")
            })
            .unwrap_err();
            assert_eq!(error.setting, "HTTPS_PROXY");
            assert_eq!(error.kind, InvalidProxy::Scheme);
            assert!(error.to_string().contains("HTTPS_PROXY"));
            assert!(!error.to_string().contains("dummy-password"));
            return;
        }

        let (port, proxy) = socks5_proxy();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let local_url = format!(
            "http://localhost:{}/",
            listener.local_addr().unwrap().port()
        );
        let origin = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.set_read_timeout(Some(TIMEOUT)).unwrap();
            let mut line = String::new();
            let mut request = BufReader::new(&stream);
            while request.read_line(&mut line).unwrap() > 2 {
                line.clear();
            }
            stream
                .write_all(
                    b"HTTP/1.1 302 Found\r\nLocation: /next\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .unwrap();
        });

        // Set the proxy only in a fresh process, never in the test runner's shared environment.
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "tests::a_configured_agent_uses_the_env_proxy_but_goes_direct_for_localhost",
            ])
            .env(LOCAL_URL, local_url)
            .env("HTTP_PROXY", format!("socks5h://127.0.0.1:{port}"))
            .env(
                "HTTPS_PROXY",
                "https://dummy-user:dummy-password@proxy.example",
            )
            .env("NO_PROXY", "")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
        assert_eq!(proxy.join().unwrap(), "sunrise.invalid");
        origin.join().unwrap();
    }

    /// The agent a second request gets is the one the first built: one connection pool, not one
    /// handshake per jar.
    #[test]
    fn an_agent_is_kept_for_the_next_request() {
        let _turn = alone();
        set_proxy(None);
        let mut built = 0;
        for _ in 0..3 {
            keep("kept", None, |_| {
                built += 1;
                ureq::AgentBuilder::new().build()
            })
            .unwrap();
        }
        assert_eq!(built, 1, "three requests, one agent");
        // Another address is another agent, and a new setting drops the ones before it.
        keep("kept", Some("http://127.0.0.1:7890"), |proxy| {
            assert!(proxy.is_some());
            built += 1;
            ureq::AgentBuilder::new().build()
        })
        .unwrap();
        assert_eq!(built, 2, "a second address needs a second agent");
        set_proxy(Some("http://127.0.0.1:7891".into()));
        keep("kept", Some("http://127.0.0.1:7891"), |_| {
            built += 1;
            ureq::AgentBuilder::new().build()
        })
        .unwrap();
        assert_eq!(built, 3, "a new setting drops the agents kept before it");
        set_proxy(None);
    }
}
