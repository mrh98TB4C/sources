use aidoku::{
	HashMap,
	alloc::{format, string::String, vec::Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
	prelude::*,
};

const CF_CLEARANCE_KEY: &str = "cfClearance";
const SESSION_COOKIES_KEY: &str = "sessionCookies";

pub fn save_cookies(cookies: &HashMap<String, String>) -> bool {
	let mut saved = false;
	if let Some(value) = cookies
		.get("cf_clearance")
		.filter(|v| !v.is_empty())
	{
		defaults_set(CF_CLEARANCE_KEY, DefaultValue::String(value.clone()));
		saved = true;
	}
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
		defaults_set(SESSION_COOKIES_KEY, DefaultValue::String(session.join("; ")));
		saved = true;
	}
	saved
}

pub fn cookie_header() -> String {
	let mut cookies = Vec::from([String::from("NMfYa=1"), String::from("nm_mobile=1")]);
	let has_cf = defaults_get::<String>(CF_CLEARANCE_KEY)
		.filter(|v| !v.is_empty())
		.is_some();
	let session = defaults_get::<String>(SESSION_COOKIES_KEY).unwrap_or_default();
	let has_session = !session.is_empty();
	println!("NudeMoon: cf={has_cf} session_len={}", session.len());
	if let Some(v) = has_cf.then(|| defaults_get::<String>(CF_CLEARANCE_KEY)).flatten() {
		cookies.push(format!("cf_clearance={v}"));
	}
	if has_session {
		cookies.push(session);
	}
	cookies.push(String::from("Domain=nude-moon.org"));
	cookies.join("; ")
}

pub fn is_authorized() -> bool {
	let session = defaults_get::<String>(SESSION_COOKIES_KEY).unwrap_or_default();
	session.contains("userToken=") || session.contains("fusion_user=")
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

	fn setup() { clear_auth(); }

	#[aidoku_test]
	fn webview_saves_non_analytics_cookies() {
		setup();
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), String::from("cf-tok"));
		c.insert(String::from("_ga"), String::from("ga-tok"));
		c.insert(String::from("userToken"), String::from("real-token"));
		c.insert(String::from("fusion_visited"), String::from("1"));
		assert!(save_cookies(&c));
		assert!(is_authorized());
		let h = cookie_header();
		assert!(h.contains("cf_clearance=cf-tok"));
		assert!(h.contains("userToken=real-token"));
		assert!(h.contains("fusion_visited=1"));
		assert!(!h.contains("_ga"));
	}

	#[aidoku_test]
	fn fusion_visited_alone_is_not_authorized() {
		setup();
		let mut c = HashMap::new();
		c.insert(String::from("fusion_visited"), String::from("yes"));
		assert!(save_cookies(&c));
		assert!(!is_authorized());
	}

	#[aidoku_test]
	fn fusion_user_is_authorized() {
		setup();
		let mut c = HashMap::new();
		c.insert(String::from("fusion_user"), String::from("123.abc"));
		assert!(save_cookies(&c));
		assert!(is_authorized());
	}

	#[aidoku_test]
	fn clear_auth_removes_all() {
		setup();
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), String::from("cf-tok"));
		c.insert(String::from("userToken"), String::from("auth"));
		save_cookies(&c);
		assert!(is_authorized());
		clear_auth();
		assert!(!is_authorized());
		assert!(!has_cloudflare_clearance());
	}
}
