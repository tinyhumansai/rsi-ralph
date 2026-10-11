//! Unit tests for tool keys and how they reach seats.

use super::*;

fn key(capability: &str, provider: &str, seats: Option<&[&str]>) -> ToolKey {
    ToolKey {
        capability: capability.into(),
        provider: provider.into(),
        secret: "AIza-test-secret".into(),
        seats: seats.map(|seats| seats.iter().map(|seat| (*seat).to_owned()).collect()),
    }
}

fn seats() -> BTreeSet<&'static str> {
    BTreeSet::from(["lead", "impl"])
}

#[test]
fn an_unknown_capability_or_provider_is_unsupported() {
    for bad in [
        key("telepathy", "gemini", None),
        key(WEB_SEARCH, "perplexity", None),
    ] {
        assert!(matches!(
            validate(std::slice::from_ref(&bad), &seats()),
            Err(Error::UnsupportedTool { capability, provider })
                if capability == bad.capability && provider == bad.provider
        ));
    }
}

#[cfg(feature = "web-search")]
#[test]
fn a_search_key_for_known_seats_is_accepted() {
    assert!(validate(&[key(WEB_SEARCH, "gemini", Some(&["lead"]))], &seats()).is_ok());
    assert!(validate(&[key(WEB_SEARCH, "brave", None)], &seats()).is_ok());
}

#[cfg(feature = "web-search")]
#[test]
fn a_key_scoped_to_an_unknown_seat_is_refused() {
    assert!(matches!(
        validate(&[key(WEB_SEARCH, "gemini", Some(&["ghost"]))], &seats()),
        Err(Error::UnknownToolSeat { seat, .. }) if seat == "ghost"
    ));
}

#[cfg(not(feature = "web-search"))]
#[test]
fn search_is_unsupported_without_the_web_search_feature() {
    assert!(matches!(
        validate(&[key(WEB_SEARCH, "gemini", None)], &seats()),
        Err(Error::UnsupportedTool { .. })
    ));
}

#[test]
fn keys_reach_their_scoped_seats_only() {
    let keys = [
        key(WEB_SEARCH, "gemini", Some(&["lead"])),
        key(WEB_SEARCH, "brave", None),
    ];
    assert_eq!(for_seat(&keys, "lead").len(), 2);
    assert_eq!(for_seat(&keys, "impl"), [keys[1].clone()]);
    assert!(tool_names(&for_seat(&keys, "impl")).contains(&"web_answer_tool".to_owned()));
    assert_eq!(tool_names(&[]), Vec::<String>::new());
    assert_eq!(tool_names(&[key("other", "x", None)]), Vec::<String>::new());
}

#[test]
fn a_search_key_enables_its_provider_on_the_direct_route() {
    let mut config = RuntimeConfig::default();
    apply(
        &mut config,
        &[
            key(WEB_SEARCH, "gemini", None),
            key(WEB_SEARCH, "brave", None),
            key("other", "exa", None),
            key(WEB_SEARCH, "unknown", None),
        ],
    );
    assert_eq!(config.search.enabled, Some(true));
    assert_eq!(
        config.search.gemini.api_key.as_deref(),
        Some("AIza-test-secret")
    );
    assert_eq!(
        config.search.brave.api_key.as_deref(),
        Some("AIza-test-secret")
    );
    assert_eq!(config.search.exa.api_key, None);
    // Default settings are enabled on the direct route; a fresh config has
    // Gemini on the managed route instead.
    let mut empty = config.search.providers.clone();
    empty.clear();
    let direct = *empty.entry(String::new()).or_default();
    assert_eq!(config.search.providers["gemini"], direct);
    assert_ne!(RuntimeConfig::default().search.providers["gemini"], direct);
    assert!(!config.search.providers.contains_key("unknown"));
    for provider in SEARCH_PROVIDERS {
        let mut config = RuntimeConfig::default();
        apply(&mut config, &[key(WEB_SEARCH, provider, None)]);
        assert!(config.search.providers.contains_key(provider), "{provider}");
    }
}

#[test]
fn debug_output_hides_the_secret() {
    let shown = format!("{:?}", key(WEB_SEARCH, "gemini", None));
    assert!(shown.contains("gemini"));
    assert!(!shown.contains("AIza-test-secret"));
}
