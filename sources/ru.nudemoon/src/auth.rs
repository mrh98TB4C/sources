use aidoku::{
	HashMap,
	alloc::{format, string::String, vec::Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
};

const CF_CLEARANCE_KEY: &str = "cfClearance";
const FUSION_USER_KEY: &str = "fusionUser";

pub fn save_cookies(cookies: &HashMap<String, String>) -> bool {
	let mut saved = false;
	if let Some(cf_clearance) = cookies
		.get("cf_clearance")
		.filter(|value| !value.is_empty())
	{
		defaults_set(CF_CLEARANCE_KEY, DefaultValue::String(cf_clearance.clone()));
		saved = true;
	}
	if let Some(fusion_user) = cookies.get("fusion_user").filter(|value| !value.is_empty()) {
		defaults_set(FUSION_USER_KEY, DefaultValue::String(fusion_user.clone()));
		saved = true;
	}
	saved
}

pub fn cookie_header() -> String {
	let mut cookies = Vec::from([String::from("NMfYa=1"), String::from("nm_mobile=1")]);
	if let Some(cf_clearance) =
		defaults_get::<String>(CF_CLEARANCE_KEY).filter(|value| !value.is_empty())
	{
		cookies.push(format!("cf_clearance={cf_clearance}"));
	}
	if let Some(fusion_user) =
		defaults_get::<String>(FUSION_USER_KEY).filter(|value| !value.is_empty())
	{
		cookies.push(format!("fusion_user={fusion_user}"));
	}
	cookies.push(String::from("Domain=nude-moon.org"));
	cookies.join("; ")
}

pub fn is_authorized() -> bool {
	defaults_get::<String>(FUSION_USER_KEY).is_some_and(|value| !value.is_empty())
}

#[expect(dead_code)]
pub fn has_cloudflare_clearance() -> bool {
	defaults_get::<String>(CF_CLEARANCE_KEY).is_some_and(|value| !value.is_empty())
}

pub fn clear_cloudflare() {
	defaults_set(CF_CLEARANCE_KEY, DefaultValue::String(String::new()));
}

pub fn clear_auth() {
	defaults_set(CF_CLEARANCE_KEY, DefaultValue::String(String::new()));
	defaults_set(FUSION_USER_KEY, DefaultValue::String(String::new()));
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn webview_cookie_lifecycle() {
		clear_auth();
		assert_eq!(
			cookie_header(),
			"NMfYa=1; nm_mobile=1; Domain=nude-moon.org"
		);

		let mut unrelated = HashMap::new();
		unrelated.insert(String::from("session"), String::from("token"));
		assert!(!save_cookies(&unrelated));

		let mut cloudflare = HashMap::new();
		cloudflare.insert(
			String::from("cf_clearance"),
			String::from("clearance-token"),
		);
		assert!(save_cookies(&cloudflare));
		assert!(has_cloudflare_clearance());
		assert!(!is_authorized());
		assert_eq!(
			cookie_header(),
			"NMfYa=1; nm_mobile=1; cf_clearance=clearance-token; Domain=nude-moon.org"
		);

		let mut account = HashMap::new();
		account.insert(String::from("fusion_user"), String::from("account-token"));
		assert!(save_cookies(&account));
		assert!(is_authorized());
		assert_eq!(
			cookie_header(),
			"NMfYa=1; nm_mobile=1; cf_clearance=clearance-token; fusion_user=account-token; Domain=nude-moon.org"
		);

		assert!(is_authorized());
		clear_cloudflare();
		assert!(!has_cloudflare_clearance());
		assert!(is_authorized());

		clear_auth();
		assert!(!has_cloudflare_clearance());
		assert!(!is_authorized());
		assert_eq!(
			cookie_header(),
			"NMfYa=1; nm_mobile=1; Domain=nude-moon.org"
		);
	}
}
