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

fn full_page() -> Vec<serde_json::Value> {
    (1000..1000 + PAGE_SIZE as u64)
        .map(|id| release(id, "nightly"))
        .collect()
}

fn page(rows: Vec<serde_json::Value>, links: &[&str]) -> Response {
    let mut response = body(rows.into());
    response.links = links.iter().map(|link| (*link).to_owned()).collect();
    response
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
            let mut rows = full_page();
            let fixtures = [
                release(2, "26.3-0.1.1"), draft, prerelease, unpublished, bad_date,
                release(7, "26.3-01.0.0"), release(8, "26.3-0.1.0-rc.1"),
                release(9, "26.3-0.1.0+build"), release(10, "26.3-0.1.18446744073709551616"),
                release(11, "nightly"), release(12, "sdk-abcdef"),
            ];
            rows.splice(..fixtures.len(), fixtures);
            Ok(page(rows, &[r#"<https://api.github.com/repositories/42/releases?per_page=100&page=2>; rel="next", <https://api.github.com/repositories/42/releases?per_page=100&page=2>; rel="last""#]))
        } else if url == format!("{SDK_RELEASE_LIST}&page=2") {
            Ok(page(vec![release(1, STABLE)], &[r#"<https://api.github.com/repositories/42/releases?page=1>; rel="prev", <https://api.github.com/repositories/42/releases?page=1>; rel="first""#]))
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
    for second in [
        Err("HTTP 503".into()),
        Ok(Response {
            body: b"[{".to_vec(),
            links: Vec::new(),
        }),
        Ok(body(json!([]))),
        Ok(page(vec![release(2, "26.3-0.1.1")], &[])),
        Ok(page(
            {
                let mut rows = full_page();
                rows[0] = release(1, STABLE);
                for (offset, row) in rows[1..].iter_mut().enumerate() {
                    *row = release(2000 + offset as u64, "nightly");
                }
                rows
            },
            &[r#"<https://api.github.com/repositories/42/releases?page=3>; rel="next""#],
        )),
    ] {
        let mut second = Some(second);
        let error = write_list(&out, |url| {
            if url == format!("{SDK_RELEASE_LIST}&page=1") {
                let mut first = full_page();
                first[0] = release(1, STABLE);
                Ok(page(first, &[r#"<https://api.github.com/repositories/42/releases?page=2>; rel="next", <https://api.github.com/repositories/42/releases?page=3>; rel="last""#]))
            } else {
                assert_eq!(url, format!("{SDK_RELEASE_LIST}&page=2"));
                second.take().expect("only one second-page request")
            }
        }).unwrap_err();
        assert!(error.contains("page 2"), "{error}");
        assert_eq!(std::fs::read(&out).unwrap(), b"keep the previous result");
    }
}

#[test]
fn an_ambiguous_first_page_keeps_the_previous_release_file() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("releases.json");
    let previous = b"unchanged result\r\n";
    std::fs::write(&out, previous).unwrap();
    let short = || vec![release(1, "nightly")];
    let link = |query: &str, relation: &str| {
        format!(r#"<https://api.github.com/repositories/42/releases?{query}>; rel="{relation}""#)
    };
    for (rows, links) in [
        (short(), vec![link("page=2", "next")]),
        (full_page(), vec![]),
        (short(), vec!["unrecognised link".into()]),
        (short(), vec![link("page=1", "unknown")]),
        (short(), vec![link("page=1", "next last")]),
        (short(), vec![link("per_page=100", "last")]),
        (short(), vec![link("page=1&page=2", "last")]),
        (
            short(),
            vec![link("page=1", "last"), link("page=1", "last")],
        ),
        (
            short(),
            vec![format!(r#"{}; rel="next""#, link("page=1", "last"))],
        ),
    ] {
        let mut response = Some(page(
            rows,
            &links.iter().map(String::as_str).collect::<Vec<_>>(),
        ));
        let error = write_list(&out, |url| {
            assert_eq!(url, format!("{SDK_RELEASE_LIST}&page=1"));
            Ok(response.take().expect("one first-page request"))
        })
        .unwrap_err();
        assert!(error.contains("page 1"), "{error}");
        assert_eq!(std::fs::read(&out).unwrap(), previous);
    }
}

#[test]
fn a_full_page_explicitly_marked_last_can_finish() {
    let rows = discover(|url| {
        assert_eq!(url, format!("{SDK_RELEASE_LIST}&page=1"));
        Ok(page(full_page(), &[r#"<https://api.github.com/repos/sunrise-org/sunrise/releases?per_page=100&page=1>; rel="last""#]))
    })
    .unwrap();
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
