use aidoku::{
	HashMap,
	alloc::{format, string::String, vec::Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
	imports::std::current_date,
};

const SESSION_KEY: &str = "session";
const SESSION_TS_KEY: &str = "session_ts";
/// cf_clearance протухает на сервере за ~30 минут; шлём только свежий.
/// Протухший отравляет запрос: CF режет его даже при живом clearance в хранилище Aidoku.
const CLEARANCE_MAX_AGE_SECS: i32 = 20 * 60;

/// Куки, которые мы вообще пересылаем в запросах. Остальное (_ga, _gid,
/// cf_chl_rc_ni, fusion_visited) — мусор, только шумит и ломает челленджи.
const FORWARD_COOKIES: [&str; 2] = ["fusion_user", "cf_clearance"];

pub fn save_cookies(cookies: &HashMap<String, String>) -> bool {
	let mut all: Vec<String> = Vec::new();
	for name in FORWARD_COOKIES {
		if let Some(value) = cookies.get(name).filter(|v| !v.is_empty()) {
			all.push(format!("{name}={value}"));
		}
	}
	if all.is_empty() {
		return false;
	}
	defaults_set(SESSION_KEY, DefaultValue::String(all.join("; ")));
	defaults_set(SESSION_TS_KEY, DefaultValue::Int(current_date() as i32));
	true
}

/// Значение куки из сессии по имени (сессия хранится строкой "k=v; k2=v2").
fn session_value(name: &str) -> Option<String> {
	let session = defaults_get::<String>(SESSION_KEY).unwrap_or_default();
	let needle = format!("{name}=");
	for part in session.split("; ") {
		if let Some(value) = part.strip_prefix(&needle) {
			return Some(value.into());
		}
	}
	None
}

pub fn cookie_header() -> String {
	let mut header = String::from("NMfYa=1; nm_mobile=1");

	if let Some(fu) = session_value("fusion_user") {
		header.push_str("; fusion_user=");
		header.push_str(&fu);
	}

	// cf_clearance только если свежий (получен из WebView недавно).
	// Протухший не шлём: он вызывает вечный челлендж-цикл.
	if let Some(clearance) = session_value("cf_clearance") {
		let saved_at = defaults_get::<i32>(SESSION_TS_KEY).unwrap_or(0) as i64;
		if current_date() - saved_at < CLEARANCE_MAX_AGE_SECS as i64 {
			header.push_str("; cf_clearance=");
			header.push_str(&clearance);
		}
	}

	let manual = defaults_get::<String>("manualCookies").unwrap_or_default();
	if !manual.is_empty() {
		header.push_str("; ");
		header.push_str(&manual);
	}

	header.push_str("; Domain=nude-moon.org");
	header
}

pub fn is_authorized() -> bool {
	// fusion_user is the ONLY account auth cookie. userToken is set for
	// anonymous visitors too, so it must not count as authorization.
	session_value("fusion_user").is_some()
		|| defaults_get::<String>("manualCookies")
			.unwrap_or_default()
			.contains("fusion_user=")
}

#[allow(dead_code)] // только для тестов; логаут управляется самим Aidoku
pub fn clear_auth() {
	defaults_set(SESSION_KEY, DefaultValue::String(String::new()));
	defaults_set(SESSION_TS_KEY, DefaultValue::Int(0));
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn saves_only_forward_cookies() {
		clear_auth();
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), String::from("cf"));
		c.insert(String::from("_ga"), String::from("ga"));
		c.insert(String::from("fusion_user"), String::from("fu"));
		assert!(save_cookies(&c));
		assert!(is_authorized());
		// мусор (_ga и т.п.) в сессию не попадает
		assert!(!defaults_get::<String>(SESSION_KEY)
			.unwrap_or_default()
			.contains("_ga"));
	}

	#[aidoku_test]
	fn is_authorized_requires_real_token() {
		clear_auth();
		assert!(!is_authorized());
		let mut c = HashMap::new();
		c.insert(String::from("x"), String::from("1"));
		save_cookies(&c);
		assert!(!is_authorized()); // arbitrary cookie != auth
		let mut c2 = HashMap::new();
		c2.insert(String::from("fusion_user"), String::from("tok"));
		save_cookies(&c2);
		assert!(is_authorized());
	}

	#[aidoku_test]
	fn clear_auth_wipes_session() {
		let mut c = HashMap::new();
		c.insert(String::from("fusion_user"), String::from("tok"));
		save_cookies(&c);
		assert!(is_authorized());
		clear_auth();
		assert!(!is_authorized());
	}

	#[aidoku_test]
	fn cookie_header_drops_stale_clearance_keeps_auth() {
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), String::from("oldcf"));
		c.insert(String::from("fusion_user"), String::from("fu"));
		save_cookies(&c);
		// состариваем метку: будто clearance получен час назад
		defaults_set(SESSION_TS_KEY, DefaultValue::Int(current_date() as i32 - 3600));
		let header = cookie_header();
		assert!(!header.contains("cf_clearance"));
		assert!(header.contains("fusion_user=fu"));
		assert!(header.contains("Domain=nude-moon.org"));
	}

	#[aidoku_test]
	fn cookie_header_includes_fresh_clearance_and_manual() {
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), String::from("freshcf"));
		save_cookies(&c);
		defaults_set("manualCookies", DefaultValue::String(String::from("fusion_user=mfu")));
		let header = cookie_header();
		assert!(header.contains("cf_clearance=freshcf"));
		assert!(header.contains("fusion_user=mfu"));
	}

	#[aidoku_test]
	fn save_cookies_rejects_empty_payload() {
		clear_auth();
		let mut c = HashMap::new();
		c.insert(String::from("_ga"), String::from("ga"));
		assert!(!save_cookies(&c));
		assert!(!is_authorized());
	}
}
