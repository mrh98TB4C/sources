use aidoku::{
	HashMap,
	alloc::{format, string::String, vec::Vec},
	imports::defaults::{DefaultValue, defaults_get, defaults_set},
	imports::std::current_date,
};

const SESSION_KEY: &str = "session";
const CF_BLOCKED_KEY: &str = "cf_blocked";

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
	// Анонимная доставка (страница грузится без логина) не должна стирать
	// fusion_user из сессии: сохраняем прежний, если в новой доставке его нет.
	if !cookies.contains_key("fusion_user")
		&& let Some(old_fu) = session_value("fusion_user")
	{
		all.push(format!("fusion_user={old_fu}"));
	}
	if all.is_empty() {
		return false;
	}
	defaults_set(SESSION_KEY, DefaultValue::String(all.join("; ")));
	// WebView побывал на сайте — если стояла пост-403 пауза, снимаем.
	clear_cf_blocked();
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

/// Время выдачи cf_clearance, вшитое CF в токен: `TOKEN-<unix>-1.2.1.1-...`
fn clearance_issued_at(value: &str) -> Option<i64> {
	value.split('-').nth(1)?.parse().ok()
}

pub fn cookie_header() -> String {
	let mut header = String::from("NMfYa=1; nm_mobile=1");

	if let Some(fu) = session_value("fusion_user") {
		header.push_str("; fusion_user=");
		header.push_str(&fu);
	}

	if let Some(clearance) = session_value("cf_clearance") {
		header.push_str("; cf_clearance=");
		header.push_str(&clearance);
	}

	header.push_str("; Domain=nude-moon.org");
	header
}

/// Токен cf_clearance из сессии (для сравнения с доставкой).
pub fn stored_clearance() -> Option<String> {
	session_value("cf_clearance")
}

/// После 403 от CF не делаем новых запросов — каждый голый запрос запускает
/// авто-челлендж Aidoku (поп-ап, повторы, краш). Пауза до доставки кук из WebView.
pub fn set_cf_blocked() {
	defaults_set(CF_BLOCKED_KEY, DefaultValue::Int(current_date() as i32));
}

pub fn clear_cf_blocked() {
	defaults_set(CF_BLOCKED_KEY, DefaultValue::Int(0));
}

pub fn is_cf_blocked() -> bool {
	defaults_get::<i32>(CF_BLOCKED_KEY).unwrap_or(0) != 0
}

/// Возраст cf_clearance, вшитый в сам токен, надёжнее нашей метки времени
/// (дефолты источника стираются при обновлениях). Токен старше порога
/// не отправляем: сайт всё равно 403-нет, а каждый 403 запускает
/// авто-челлендж Aidoku (поп-апы, краш).
const CLEARANCE_MAX_AGE_SECS: i64 = 25 * 60;

pub fn clearance_is_usable() -> bool {
	match session_value("cf_clearance") {
		None => false,
		Some(c) => match clearance_issued_at(&c) {
			Some(issued) => current_date() - issued < CLEARANCE_MAX_AGE_SECS,
			// Токен без вшитой метки — отправляем: хуже один 403, чем вечный стоп.
			None => true,
		},
	}
}

/// Есть ли в сессии cf_clearance (без учёта возраста).
pub fn has_clearance() -> bool {
	session_value("cf_clearance").is_some()
}

pub fn is_authorized() -> bool {
	// fusion_user is the ONLY account auth cookie. userToken is set for
	// anonymous visitors too, so it must not count as authorization.
	session_value("fusion_user").is_some()
}

/// Диагностика: что в сессии, сколько времени токену (по вшитой CF метке).
pub fn diag() -> String {
	let clearance = session_value("cf_clearance")
		.map(|c| c.chars().take(12).collect::<String>())
		.unwrap_or_else(|| String::from("none"));
	let age = session_value("cf_clearance")
		.and_then(|c| clearance_issued_at(&c))
		.map(|issued| current_date() - issued)
		.unwrap_or(-1);
	let auth = if session_value("fusion_user").is_some() { "yes" } else { "no" };
	format!("session(auth={auth}, clearance={clearance}, token_age={age}s)")
}

#[allow(dead_code)] // только для тестов; логаут управляется самим Aidoku
pub fn clear_auth() {
	defaults_set(SESSION_KEY, DefaultValue::String(String::new()));
	defaults_set(CF_BLOCKED_KEY, DefaultValue::Int(0));
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
	fn anonymous_delivery_keeps_existing_fusion_user() {
		clear_auth();
		let mut c = HashMap::new();
		c.insert(String::from("fusion_user"), String::from("fu"));
		save_cookies(&c);
		// анонимная доставка: только clearance
		let mut anon = HashMap::new();
		anon.insert(String::from("cf_clearance"), String::from("cf2"));
		save_cookies(&anon);
		assert!(is_authorized());
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
	fn cookie_header_sends_clearance_regardless_of_age() {
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), String::from("oldcf"));
		c.insert(String::from("fusion_user"), String::from("fu"));
		save_cookies(&c);
		let header = cookie_header();
		assert!(header.contains("cf_clearance=oldcf"));
		assert!(header.contains("fusion_user=fu"));
		assert!(header.contains("Domain=nude-moon.org"));
	}

	#[aidoku_test]
	fn clearance_issued_at_reads_embedded_timestamp() {
		let tok = "abc.xyz-1786775925-1.2.1.1-nonce";
		assert_eq!(clearance_issued_at(tok), Some(1786775925));
		assert_eq!(clearance_issued_at("garbage"), None);
	}

	#[aidoku_test]
	fn clearance_usable_by_embedded_age() {
		clear_auth();
		assert!(!clearance_is_usable()); // нет токена
		let fresh = format!("tok-{}-1.2.1.1-nonce", current_date());
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), fresh);
		save_cookies(&c);
		assert!(clearance_is_usable()); // свежий токен
		let old = format!("tok-{}-1.2.1.1-nonce", current_date() - 3600);
		let mut c2 = HashMap::new();
		c2.insert(String::from("cf_clearance"), old);
		save_cookies(&c2);
		assert!(!clearance_is_usable()); // часовой токен не отправляем
		let mut c3 = HashMap::new();
		c3.insert(String::from("cf_clearance"), String::from("no-timestamp"));
		save_cookies(&c3);
		assert!(clearance_is_usable()); // без метки — отправляем
	}

	#[aidoku_test]
	fn cf_blocked_flag_clears_on_delivery() {
		clear_auth();
		set_cf_blocked();
		assert!(is_cf_blocked());
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), String::from("cf"));
		save_cookies(&c);
		assert!(!is_cf_blocked());
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
