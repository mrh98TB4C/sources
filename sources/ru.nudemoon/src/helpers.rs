use aidoku::helpers::uri::encode_uri_component;
use alloc::{format, string::String, vec::Vec};
use core::fmt::Write;

const BASE_URL: &str = "https://nude-moon.org";
const PAGE_SIZE: i32 = 30;

pub fn search_url(query: &str, page: i32) -> String {
	format!(
		"{BASE_URL}/search?stext={}&rowstart={}",
		encode_cp1251(query),
		PAGE_SIZE * (page - 1)
	)
}

pub fn browse_url(page: i32, sort_index: usize, genres: &[String]) -> String {
	let rowstart = PAGE_SIZE * (page - 1);
	if genres.is_empty() {
		let order = match sort_index {
			0 => "date",
			2 => "like",
			_ => "views",
		};
		format!("{BASE_URL}/all_manga?{order}&rowstart={rowstart}")
	} else {
		let order = match sort_index {
			0 => "&date",
			2 => "&like",
			_ => "&views",
		};
		format!(
			"{BASE_URL}/tags/{}{order}&rowstart={rowstart}",
			genres
				.iter()
				.map(encode_uri_component)
				.collect::<Vec<_>>()
				.join("+")
		)
	}
}

pub fn url_key(url: &str) -> Option<String> {
	if url.is_empty() {
		return None;
	}
	if url.starts_with('/') {
		return Some(url.into());
	}
	if let Some(scheme_end) = url.find("://") {
		let after_scheme = &url[scheme_end + 3..];
		if let Some(path_start) = after_scheme.find('/') {
			return Some(after_scheme[path_start..].into());
		}
		return None;
	}
	Some(url.into())
}

pub fn clean_title(title: &str) -> String {
	title
		.split(" / ")
		.next()
		.unwrap_or(title)
		.split(" №")
		.next()
		.unwrap_or(title)
		.trim()
		.into()
}

pub fn chapter_number(title: &str) -> Option<f32> {
	let index = title.find('№')? + '№'.len_utf8();
	let number: String = title[index..]
		.chars()
		.take_while(|c| c.is_ascii_digit() || *c == '.')
		.collect();
	number.parse().ok()
}

fn encode_cp1251(input: &str) -> String {
	let mut encoded = String::new();
	for c in input.chars() {
		match c {
			' ' => encoded.push('+'),
			'-' | '.' | '*' | '_' | '0'..='9' | 'A'..='Z' | 'a'..='z' => encoded.push(c),
			_ => {
				let byte = cp1251_byte(c).unwrap_or(b'?');
				write!(&mut encoded, "%{byte:02X}").ok();
			}
		}
	}
	encoded
}

fn cp1251_byte(c: char) -> Option<u8> {
	match c {
		'\u{0000}'..='\u{007F}' => Some(c as u8),
		'Ё' => Some(0xA8),
		'ё' => Some(0xB8),
		'А'..='я' => Some((c as u32 - 'А' as u32 + 0xC0) as u8),
		'–' => Some(0x96),
		'—' => Some(0x97),
		'‘' => Some(0x91),
		'’' => Some(0x92),
		'“' => Some(0x93),
		'”' => Some(0x94),
		'«' => Some(0xAB),
		'»' => Some(0xBB),
		'№' => Some(0xB9),
		_ => None,
	}
}

pub fn decode_cp1251(data: &[u8]) -> String {
	data.iter().map(|&b| decode_cp1251_byte(b)).collect()
}

fn decode_cp1251_byte(byte: u8) -> char {
	match byte {
		0x00..=0x7F => byte as char,
		0xA8 => 'Ё',
		0xB8 => 'ё',
		0x96 => '\u{2013}',
		0x97 => '\u{2014}',
		0x91 => '\u{2018}',
		0x92 => '\u{2019}',
		0x93 => '\u{201C}',
		0x94 => '\u{201D}',
		0xAB => '«',
		0xBB => '»',
		0xB9 => '№',
		0xC0..=0xDF => char::from_u32(0x0410 + (byte - 0xC0) as u32).unwrap_or('?'),
		0xE0..=0xFF => char::from_u32(0x0430 + (byte - 0xE0) as u32).unwrap_or('?'),
		_ => '?',
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn search_url_uses_cp1251_encoding() {
		assert_eq!(
			search_url("тест", 1),
			"https://nude-moon.org/search?stext=%F2%E5%F1%F2&rowstart=0"
		);
		assert_eq!(
			search_url("секс игрушки", 2),
			"https://nude-moon.org/search?stext=%F1%E5%EA%F1+%E8%E3%F0%F3%F8%EA%E8&rowstart=30"
		);
	}

	#[aidoku_test]
	fn browse_url_uses_views_by_default() {
		assert_eq!(
			browse_url(2, 1, &[]),
			"https://nude-moon.org/all_manga?views&rowstart=30"
		);
	}

	#[aidoku_test]
	fn browse_url_combines_genres_and_sort() {
		let genres = Vec::from([String::from("без_цензуры"), String::from("x-ray")]);
		assert_eq!(
			browse_url(1, 0, &genres),
			"https://nude-moon.org/tags/%D0%B1%D0%B5%D0%B7_%D1%86%D0%B5%D0%BD%D0%B7%D1%83%D1%80%D1%8B+x-ray&date&rowstart=0"
		);
	}

	#[aidoku_test]
	fn url_key_accepts_absolute_and_relative_urls() {
		assert_eq!(
			url_key("https://nude-moon.org/manga/123-example?foo=bar"),
			Some(String::from("/manga/123-example?foo=bar"))
		);
		assert_eq!(
			url_key("/manga/123-example"),
			Some(String::from("/manga/123-example"))
		);
		assert_eq!(
			url_key("https://mirror.example.org/manga/123"),
			Some(String::from("/manga/123"))
		);
		assert_eq!(url_key("manga/123"), Some(String::from("manga/123")));
		assert_eq!(url_key(""), None);
	}

	#[aidoku_test]
	fn clean_title_removes_translation_and_number_suffixes() {
		assert_eq!(
			clean_title("Example Title / Русский"),
			String::from("Example Title")
		);
		assert_eq!(
			clean_title("Example Title №12"),
			String::from("Example Title")
		);
	}

	#[aidoku_test]
	fn chapter_number_reads_number_marker() {
		assert_eq!(chapter_number("Example Title №12.5 часть"), Some(12.5));
		assert_eq!(chapter_number("Example Title"), None);
	}

	#[aidoku_test]
	fn decode_cp1251_converts_cyrillic() {
		// "тест" in windows-1251: F2 E5 F1 F2
		let decoded = decode_cp1251(&[0xF2, 0xE5, 0xF1, 0xF2]);
		assert_eq!(decoded, "тест");
	}
}
