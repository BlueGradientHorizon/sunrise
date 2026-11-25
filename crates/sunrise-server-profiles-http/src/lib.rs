//! Online Minecraft Services account lookups and public-key fetches over HTTPS:
//! `MinecraftServicesProfileRepository.findProfileByName`,
//! `YggdrasilMinecraftSessionService.fetchProfile`, and `MinecraftServicesKeyInfo.get`.
#![forbid(unsafe_code)]

use std::time::Duration;
use sunrise_server_files::{
    json::{self, Json},
    profiles::{ServicesKeySet, discovery_url, found, profile_key_keys, valid_player_name},
    user_lists::NameAndId,
};

/// How a server resolves a name it was given (CachedUserNameToIdResolver.get,
/// without the usercache.json cache): an offline server makes the offline
/// profile; an online one asks the services (for the name in lower case) and
/// knows nobody they don't.
pub fn resolve(name: &str, online: bool) -> Option<NameAndId> {
    if online && valid_player_name(name) {
        find_by_name(&discovery_url(), &name.to_ascii_lowercase())
    } else {
        (!online).then(|| NameAndId::offline(name))
    }
}

/// MinecraftServicesProfileRepository.findProfileByName: the account named
/// `name` (any case), or None when there is none or the services don't
/// answer (vanilla logs both as "Couldn't find profile with name").
pub fn find_by_name(discovery: &str, name: &str) -> Option<NameAndId> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .user_agent(&format!("Sunrise/{}", sunrise_core::release::VERSION))
        .build();
    let doc = json::parse(&agent.get(discovery).call().ok()?.into_string().ok()?, true).ok()?;
    let url = endpoint(&doc, "profiles", "getByName")?.replace("{name}", &name.to_lowercase());
    found(&agent.get(&url).call().ok()?.into_string().ok()?, name)
}

/// YggdrasilMinecraftSessionService.fetchProfile(id, true), as the management protocol finds a player given by
/// id that the user cache doesn't know: the `session` / `getProfileById` endpoint, or None.
pub fn find_by_id(discovery: &str, id: u128) -> Option<NameAndId> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .user_agent(&format!("Sunrise/{}", sunrise_core::release::VERSION))
        .build();
    let doc = json::parse(&agent.get(discovery).call().ok()?.into_string().ok()?, true).ok()?;
    let url =
        endpoint(&doc, "session", "getProfileById")?.replace("{profileId}", &format!("{id:032x}"));
    let separator = if url.contains('?') { '&' } else { '?' };
    let answer = agent
        .get(&format!("{url}{separator}unsigned=false"))
        .call()
        .ok()?
        .into_string()
        .ok()?;
    let name = json::parse(&answer, true).ok()?.get("name")?.as_string()?;
    found(&answer, &name).filter(|user| user.id == id)
}

/// MinecraftServicesKeyInfo.get: the services' public keys from the
/// discovery document's `authentication` / `getPublicKeys` endpoint.
/// The error is the line vanilla logs, after which it has no keys.
pub fn fetch_profile_key_keys(discovery: &str) -> Result<ServicesKeySet, &'static str> {
    const FAILED: &str = "Failed to request Minecraft Services public key";
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(15))
        .user_agent(&format!("Sunrise/{}", sunrise_core::release::VERSION))
        .build();
    let get = |url: &str| agent.get(url).call().ok()?.into_string().ok();
    let doc = get(discovery)
        .and_then(|d| json::parse(&d, true).ok())
        .ok_or(FAILED)?;
    let url = endpoint(&doc, "authentication", "getPublicKeys").ok_or(FAILED)?;
    profile_key_keys(&get(&url).ok_or(FAILED)?)
}

/// MinecraftServicesDiscoveryService.getUrl(service, endpoint).
fn endpoint(doc: &Json, service: &str, name: &str) -> Option<String> {
    doc.get("discovery")?
        .get(service)?
        .get("endpoints")?
        .get(name)?
        .get("uri")?
        .as_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_offline_and_invalid_names() {
        assert_eq!(resolve("Alex", false), Some(NameAndId::offline("Alex")));
        assert_eq!(resolve("not valid", true), None);
    }
}
