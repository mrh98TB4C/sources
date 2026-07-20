use aidoku::helpers::uri::encode_uri_component;
use alloc::{format, string::String, vec::Vec};

const BASE_URL: &str = "https://acomics.ru";
const PAGE_SIZE: i32 = 10;

pub fn search_url(query: &str) -> String {
	format!("{BASE_URL}/search?keyword={}", encode_uri_component(query))
}

pub fn browse_url(page: i32, key_values: &[(String, String)]) -> String {
	let skip = PAGE_SIZE * (page - 1);
	let mut pairs: Vec<String> = key_values
		.iter()
		.map(|(k, v)| format!("{k}={}", encode_uri_component(v)))
		.collect();
	pairs.push(format!("skip={skip}"));
	format!("{BASE_URL}/comics?{}", pairs.join("&"))
}

pub fn manga_key(href: &str) -> Option<String> {
	if href.is_empty() || !href.starts_with("/~") {
		return None;
	}
	Some(href.into())
}

pub fn chapter_url(key: &str, issue: i32) -> String {
	format!("{BASE_URL}{key}/{issue}")
}

pub fn details_url(key: &str) -> String {
	format!("{BASE_URL}{key}/about")
}

pub fn category_id(name: &str) -> Option<&'static str> {
	let idx = CATEGORIES.iter().position(|&c| c == name)?;
	Some(CATEGORY_IDS[idx])
}

pub fn rating_id(name: &str) -> Option<&'static str> {
	let idx = RATINGS.iter().position(|&r| r == name)?;
	Some(RATING_IDS[idx])
}

static CATEGORIES: [&str; 15] = [
	"Животные",
	"Драма",
	"Фентези",
	"Игры",
	"Юмор",
	"Журнал",
	"Паранормальное",
	"Конец света",
	"Романтика",
	"Фантастика",
	"Бытовое",
	"Стимпанк",
	"Супергерои",
	"Детектив",
	"Историческое",
];

static CATEGORY_IDS: [&str; 15] = [
	"1", "2", "3", "4", "5", "6", "7", "8", "9", "10", "11", "12", "13", "14", "15",
];

static RATINGS: [&str; 6] = ["NR", "G", "PG", "PG-13", "R", "NC-17"];
static RATING_IDS: [&str; 6] = ["1", "2", "3", "4", "5", "6"];

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn search_url_encodes_query() {
		assert_eq!(
			search_url("тест"),
			"https://acomics.ru/search?keyword=%D1%82%D0%B5%D1%81%D1%82"
		);
	}

	#[aidoku_test]
	fn browse_url_builds_query_params() {
		let params = Vec::from([
			(String::from("sort"), String::from("subscr_count")),
			(String::from("type"), String::from("0")),
		]);
		assert_eq!(
			browse_url(2, &params),
			"https://acomics.ru/comics?sort=subscr_count&type=0&skip=10"
		);
	}

	#[aidoku_test]
	fn browse_url_includes_categories_array() {
		let params: Vec<(String, String)> = Vec::from([
			("categories[]".into(), "1".into()),
			("categories[]".into(), "2".into()),
			("sort".into(), "subscr_count".into()),
		]);
		assert_eq!(
			browse_url(1, &params),
			"https://acomics.ru/comics?categories[]=1&categories[]=2&sort=subscr_count&skip=0"
		);
	}

	#[aidoku_test]
	fn manga_key_extracts_path() {
		assert_eq!(manga_key("/~LwHG"), Some(String::from("/~LwHG")));
		assert_eq!(manga_key(""), None);
		assert_eq!(manga_key("/comics"), None);
	}

	#[aidoku_test]
	fn category_id_maps_names() {
		assert_eq!(category_id("Животные"), Some("1"));
		assert_eq!(category_id("Фантастика"), Some("10"));
		assert_eq!(category_id("Несуществующее"), None);
	}

	#[aidoku_test]
	fn rating_id_maps_names() {
		assert_eq!(rating_id("NR"), Some("1"));
		assert_eq!(rating_id("NC-17"), Some("6"));
		assert_eq!(rating_id("X"), None);
	}

	#[aidoku_test]
	fn chapter_url_forms_correctly() {
		assert_eq!(chapter_url("/~LwHG", 42), "https://acomics.ru/~LwHG/42");
	}

	#[aidoku_test]
	fn details_url_appends_about() {
		assert_eq!(details_url("/~LwHG"), "https://acomics.ru/~LwHG/about");
	}
}
