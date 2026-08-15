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
	let mut new_clearance = false;
	for name in FORWARD_COOKIES {
		if let Some(value) = cookies.get(name).filter(|v| !v.is_empty()) {
			all.push(format!("{name}={value}"));
			if name == "cf_clearance" {
				// Сайт выдаёт новый токен при каждом челлендже; тот же токен = протухший.
				new_clearance = session_value("cf_clearance").as_deref() != Some(value.as_str());
			}
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
	// Метку времени обновляем только если clearance реально новый — иначе
	// протухший токен с новой меткой прорвётся в запрос и снова 403.
	if new_clearance {
		defaults_set(SESSION_TS_KEY, DefaultValue::Int(current_date() as i32));
	}
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
	if clearance_is_fresh()
		&& let Some(clearance) = session_value("cf_clearance")
	{
		header.push_str("; cf_clearance=");
		header.push_str(&clearance);
	}

	header.push_str("; Domain=nude-moon.org");
	header
}

/// Свежий ли cf_clearance в сессии. Источник не делает HTML-запросов без
/// свежего clearance: голый запрос 403-ится, что запускает авто-челлендж
/// Aidoku (поп-ап, цикл, краш приложения). Пауза безопаснее.
pub fn clearance_is_fresh() -> bool {
	if session_value("cf_clearance").is_none() {
		return false;
	}
	let saved_at = defaults_get::<i32>(SESSION_TS_KEY).unwrap_or(0) as i64;
	current_date() - saved_at < CLEARANCE_MAX_AGE_SECS as i64
}

pub fn is_authorized() -> bool {
	// fusion_user is the ONLY account auth cookie. userToken is set for
	// anonymous visitors too, so it must not count as authorization.
	session_value("fusion_user").is_some()
}

/// Диагностика паузы: что в сессии и сколько времени прошло.
pub fn diag() -> String {
	let clearance = session_value("cf_clearance")
		.map(|c| c.chars().take(12).collect::<String>())
		.unwrap_or_else(|| String::from("none"));
	let ts = defaults_get::<i32>(SESSION_TS_KEY).unwrap_or(0) as i64;
	let age = if ts == 0 { -1 } else { current_date() - ts };
	let auth = if session_value("fusion_user").is_some() { "yes" } else { "no" };
	format!("session(auth={auth}, clearance={clearance}, age={age}s, fresh={})", clearance_is_fresh())
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
	fn save_cookies_does_not_refresh_ts_for_same_clearance() {
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), String::from("tok1"));
		save_cookies(&c);
		defaults_set(SESSION_TS_KEY, DefaultValue::Int(current_date() as i32 - 3600));
		assert!(!clearance_is_fresh());
		// Повторная доставка того же токена не должна оживить метку
		let mut c2 = HashMap::new();
		c2.insert(String::from("cf_clearance"), String::from("tok1"));
		save_cookies(&c2);
		assert!(!clearance_is_fresh());
		// Новый токен — оживляет
		let mut c3 = HashMap::new();
		c3.insert(String::from("cf_clearance"), String::from("tok2"));
		save_cookies(&c3);
		assert!(clearance_is_fresh());
	}

	#[aidoku_test]
	fn cookie_header_includes_fresh_clearance() {
		let mut c = HashMap::new();
		c.insert(String::from("cf_clearance"), String::from("freshcf"));
		save_cookies(&c);
		let header = cookie_header();
		assert!(header.contains("cf_clearance=freshcf"));
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
