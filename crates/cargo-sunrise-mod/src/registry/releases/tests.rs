use super::*;
use serde_json::json;

const A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "cccccccccccccccccccccccccccccccccccccccc";
const D: &str = "dddddddddddddddddddddddddddddddddddddddd";
const STABLE: &str = "26.3-0.1.0";

fn release(id: u64, tag: &str) -> serde_json::Value {
    json!({
        "id": id, "tag_name": tag, "draft": false, "prerelease": false,
        "published_at": "2026-11-21T03:04:05Z",
        "target_commitish": "main"
    })
}

fn body(value: serde_json::Value) -> Response {
    Response {
        body: serde_json::to_vec(&value).unwrap(),
        links: Vec::new(),
    }
}

fn object(kind: &str, sha: &str) -> Response {
    body(json!({"object": {"type": kind, "sha": sha}}))
}

#[test]
fn stable_releases_are_filtered_sorted_and_peeled_across_pages() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("releases.json");
    let mut requests = Vec::new();
    write_list(&out, |url| {
        requests.push(url.to_owned());
        if url == format!("{SDK_RELEASE_LIST}&page=1") {
            let mut draft = release(3, "26.3-0.1.2");
            draft["draft"] = true.into();
            let mut prerelease = release(4, "26.3-0.1.3");
            prerelease["prerelease"] = true.into();
            let mut unpublished = release(5, "26.3-0.2.0");
            unpublished["published_at"] = serde_json::Value::Null;
            let mut bad_date = release(6, "26.3-0.3.0");
            bad_date["published_at"] = "2026-11-21".into();
            let mut page = body(json!([
                release(2, "26.3-0.1.1"), draft, prerelease, unpublished, bad_date,
                release(7, "26.3-01.0.0"), release(8, "26.3-0.1.0-rc.1"),
                release(9, "26.3-0.1.0+build"), release(10, "26.3-0.1.18446744073709551616"),
                release(11, "nightly"), release(12, "sdk-abcdef")
            ]));
            // GitHub can use a repository ID in a Link. Only the page number is used.
            page.links.push("<https://api.github.com/repositories/42/releases?per_page=100&page=2>; rel=\"next\", <https://api.github.com/repositories/42/releases?per_page=100&page=2>; rel=\"last\"".into());
            Ok(page)
        } else if url == format!("{SDK_RELEASE_LIST}&page=2") {
            Ok(body(json!([release(1, STABLE)])))
        } else if url == format!("{GAME_GIT_API}/ref/tags/{STABLE}") {
            Ok(object("commit", A))
        } else if url == format!("{GAME_GIT_API}/ref/tags/26.3-0.1.1") {
            Ok(object("tag", B))
        } else if url == format!("{GAME_GIT_API}/tags/{B}") {
            Ok(object("tag", C))
        } else if url == format!("{GAME_GIT_API}/tags/{C}") {
            Ok(object("commit", D))
        } else {
            panic!("unexpected request: {url}");
        }
    }).unwrap();
    assert_eq!(requests.len(), 6);
    assert_eq!(
        std::fs::read_to_string(out).unwrap(),
        format!(
            "[{{\"tag\":\"26.3-0.1.0\",\"commit\":\"{A}\"}},{{\"tag\":\"26.3-0.1.1\",\"commit\":\"{D}\"}}]\n"
        )
    );
}

#[test]
fn an_incomplete_listing_does_not_replace_a_previous_file() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("releases.json");
    std::fs::write(&out, "keep the previous result").unwrap();
    for (second, expected) in [
        (Err("HTTP 503".into()), "HTTP 503"),
        (
            Ok(Response {
                body: b"[{".to_vec(),
                links: Vec::new(),
            }),
            "EOF",
        ),
        (Ok(body(json!([]))), "incomplete"),
        (Ok(body(json!([release(1, STABLE)]))), "appeared twice"),
        (
            Ok(body(json!([release(2, "26.3-0.1.1")]))),
            "missing next page",
        ),
    ] {
        let mut second = Some(second);
        let error = write_list(&out, |url| {
            if url == format!("{SDK_RELEASE_LIST}&page=1") {
                let mut result = body(json!([release(1, STABLE)]));
                result.links.push("<https://api.github.com/repositories/42/releases?page=2>; rel=\"next\", <https://api.github.com/repositories/42/releases?page=3>; rel=\"last\"".into());
                Ok(result)
            } else {
                assert_eq!(url, format!("{SDK_RELEASE_LIST}&page=2"));
                second.take().expect("only one second-page request")
            }
        }).unwrap_err();
        assert!(
            error.contains("page 2") && error.contains(expected),
            "{error}"
        );
        assert_eq!(
            std::fs::read_to_string(&out).unwrap(),
            "keep the previous result"
        );
    }
}

#[test]
fn a_full_page_without_links_is_not_silently_truncated() {
    let mut calls = 0;
    let rows = discover(|url| {
        calls += 1;
        match calls {
            1 => {
                assert_eq!(url, format!("{SDK_RELEASE_LIST}&page=1"));
                Ok(body(
                    (0..PAGE_SIZE as u64)
                        .map(|id| release(id, "nightly"))
                        .collect(),
                ))
            }
            2 => {
                assert_eq!(url, format!("{SDK_RELEASE_LIST}&page=2"));
                Ok(body(json!([])))
            }
            _ => panic!("extra request to {url}"),
        }
    })
    .unwrap();
    assert_eq!(calls, 2);
    assert!(rows.is_empty());
}

#[test]
fn a_tag_must_end_at_a_full_commit_and_cannot_cycle() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("releases.json");
    for (kind, sha, reason) in [
        ("tag", B, "cycle"),
        ("tree", A, "does not point to a commit"),
        ("commit", "main", "full lowercase Git object ID"),
    ] {
        let error = write_list(&out, |url| {
            if url == format!("{SDK_RELEASE_LIST}&page=1") {
                Ok(body(json!([release(1, STABLE)])))
            } else {
                assert!(
                    url == format!("{GAME_GIT_API}/ref/tags/{STABLE}")
                        || url == format!("{GAME_GIT_API}/tags/{B}")
                );
                Ok(object(kind, sha))
            }
        })
        .unwrap_err();
        assert!(error.contains(STABLE) && error.contains(reason), "{error}");
        assert!(!out.exists());
    }
}
