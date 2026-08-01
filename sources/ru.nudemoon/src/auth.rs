use aidoku::{
	HashMap,
	alloc::{format, string::String, vec::Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const CF_CLEARANCE_KEY: &str = "cfClearance";
const SESSION_COOKIES_KEY: &str = "sessionCookies";

/// Save cookies from a WebView login session.
/// Stores Cloudflare clearance separately; all other non-analytics
/// cookies are persisted as a session blob and sent with every request.
pub fn save_cookies(cookies: &HashMap<String, String>) -> bool {
	let mut saved = false;

	// Cloudflare clearance
	if let Some(value) = cookies.get("cf_clearance").filter(|v| !v.is_empty()) {
		defaults_set(CF_CLEARANCE_KEY, DefaultValue::String(value.clone()));
		saved = true;
	}

	// All non-analytics cookies — the site may use any name for auth tokens.
	let session: Vec<String> = cookies
		.iter()
		.filter(|(name, value)| {
			!value.is_empty()
				&& *name != "cf_clearance"
				&& !name.starts_with("_ga")
				&& !name.starts_with("_gid")
				&& !name.starts_with("_gat")
		})
		.map(|(name, value)| format!("{name}={value}"))
		.collect();

	if !session.is_empty() {
		defaults_set(
			SESSION_COOKIES_KEY,
			DefaultValue::String(session.join("; ")),
		);
		saved = true;
	}

	saved
}

pub fn cookie_header() -> String {
	let mut cookies = Vec::from([String::from("NMfYa=1"), String::from("nm_mobile=1")]);

	if let Some(cf) = defaults_get::<String>(CF_CLEARANCE_KEY).filter(|v| !v.is_empty()) {
		cookies.push(format!("cf_clearance={cf}"));
	}

	if let Some(session) = defaults_get::<String>(SESSION_COOKIES_KEY).filter(|v| !v.is_empty()) {
		cookies.push(session);
	}

	cookies.push(String::from("Domain=nude-moon.org"));
	cookies.join("; ")
}

pub fn is_authorized() -> bool {
	defaults_get::<String>(SESSION_COOKIES_KEY).is_some_and(|v| !v.is_empty())
}

#[expect(dead_code)]
pub fn has_cloudflare_clearance() -> bool {
	defaults_get::<String>(CF_CLEARANCE_KEY).is_some_and(|value| !value.is_empty())
}

pub fn clear_cloudflare() {
	defaults_set(CF_CLEARANCE_KEY, DefaultValue::String(String::new()));
}

#[allow(dead_code)]
pub fn clear_auth() {
	defaults_set(CF_CLEARANCE_KEY, DefaultValue::String(String::new()));
	defaults_set(SESSION_COOKIES_KEY, DefaultValue::String(String::new()));
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	fn setup() {
		clear_auth();
	}

	#[aidoku_test]
	fn webview_saves_non_analytics_cookies() {
		setup();

		let mut cookies = HashMap::new();
		cookies.insert(String::from("cf_clearance"), String::from("cf-tok"));
		cookies.insert(String::from("_ga"), String::from("ga-tok"));
		cookies.insert(String::from("_gid"), String::from("gid-tok"));
		cookies.insert(String::from("_gat_gtag_123"), String::from("1"));
		cookies.insert(String::from("userToken"), String::from("real-auth-token"));
		cookies.insert(String::from("fusion_visited"), String::from("1"));

		assert!(save_cookies(&cookies));
		assert!(is_authorized());
		assert!(has_cloudflare_clearance());

		let header = cookie_header();
		assert!(header.contains("cf_clearance=cf-tok"));
		assert!(header.contains("userToken=real-auth-token"));
		assert!(header.contains("fusion_visited=1"));
		assert!(!header.contains("_ga"));
		assert!(!header.contains("_gid"));
		assert!(!header.contains("_gat"));
	}

	#[aidoku_test]
	fn not_authorized_without_session_cookies() {
		setup();

		let mut cookies = HashMap::new();
		cookies.insert(String::from("cf_clearance"), String::from("cf-tok"));

		assert!(save_cookies(&cookies));
		assert!(!is_authorized());
		assert!(has_cloudflare_clearance());
	}

	#[aidoku_test]
	fn clear_auth_removes_all() {
		setup();

		let mut cookies = HashMap::new();
		cookies.insert(String::from("cf_clearance"), String::from("cf-tok"));
		cookies.insert(String::from("userToken"), String::from("auth"));
		save_cookies(&cookies);
		assert!(is_authorized());

		clear_auth();
		assert!(!is_authorized());
		assert!(!has_cloudflare_clearance());
	}
}
