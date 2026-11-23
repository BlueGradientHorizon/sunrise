use super::*;

#[test]
fn an_attempt_reserves_the_next_24_hours() {
    let cache = tempfile::tempdir().unwrap();
    let now = SystemTime::now();
    assert!(claim_notice_attempt(cache.path(), now).is_some());
    let stamp = cache.path().join("mod-tool-update-check");
    let written = stamp.metadata().unwrap().modified().unwrap();
    assert!(claim_notice_attempt(cache.path(), written + Duration::from_secs(86_399)).is_none());
    assert!(claim_notice_attempt(cache.path(), written + Duration::from_secs(86_400)).is_some());
}

#[test]
fn only_a_different_full_commit_is_new_to_the_tool() {
    let tool = Tool::read(br#"{"commit":"47dccbf195d32d65a184d07b75a7c5cbe63c6ac9"}"#).unwrap();
    assert!(!tool.differs_from("47dccbf195d32d65a184d07b75a7c5cbe63c6ac9"));
    assert!(tool.differs_from("db7352509f10d83df9f026f4faf7d82e456d5525"));
    assert!(Tool::read(br#"{"commit":"47dccbf"}"#).is_err());
}

#[test]
fn only_release_tools_outside_ci_and_not_offline_check_for_updates() {
    let build = "47dccbf195d32d65a184d07b75a7c5cbe63c6ac9";
    assert!(checks_allowed(false, false, build));
    assert!(!checks_allowed(true, false, build));
    assert!(!checks_allowed(false, true, build));
    assert!(!checks_allowed(false, false, "local"));
}
