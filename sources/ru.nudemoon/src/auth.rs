use aidoku::{
	HashMap,
	alloc::{format, string::String, vec::Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const SESSION_KEY: &str = "session";

pub fn save_cookies(cookies: &HashMap<String, String>) -> bool {
	let all: Vec<String> = cookies
		.iter()
		.filter(|(_, v)| !v.is_empty())
		.map(|(k, v)| format!("{k}={v}"))
		.collect();
	if all.is_empty() {
		return false;
	}
	defaults_set(SESSION_KEY, DefaultValue::String(all.join("; ")));
	true
}

pub fn is_authorized() -> bool {
	!defaults_get::<String>(SESSION_KEY)
		.unwrap_or_default()
		.is_empty()
}

pub fn clear_cloudflare() {}

#[allow(dead_code)]
pub fn clear_auth() {
	defaults_set(SESSION_KEY, DefaultValue::String(String::new()));
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn saves_all_cookies() {
		clear_auth();
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), String::from("cf"));
		c.insert(String::from("_ga"), String::from("ga"));
		c.insert(String::from("fusion_user"), String::from("fu"));
		assert!(save_cookies(&c));
		assert!(is_authorized());
	}

	#[aidoku_test]
	fn is_authorized_with_any_cookie() {
		clear_auth();
		assert!(!is_authorized());
		let mut c = HashMap::new();
		c.insert(String::from("x"), String::from("1"));
		save_cookies(&c);
		assert!(is_authorized());
	}

	#[aidoku_test]
	fn clear_auth_wipes_session() {
		let mut c = HashMap::new();
		c.insert(String::from("x"), String::from("1"));
		save_cookies(&c);
		assert!(is_authorized());
		clear_auth();
		assert!(!is_authorized());
	}
}
