use crate::BASE_URL;
use aidoku::{
	alloc::{String, Vec},
	prelude::*,
};

fn key_for_section(url: &str, section: &str) -> Option<String> {
	let path = url.split('?').next().unwrap_or(url);
	let marker = format!("/{section}/");
	path.find(&marker).map(|index| path[index..].into())
}

pub fn manga_key(url: &str) -> Option<String> {
	key_for_section(url, "manga")
}

pub fn reader_url(chapter_key: &str) -> String {
	let key = key_for_section(chapter_key, "online")
		.or_else(|| manga_key(chapter_key).map(|key| key.replacen("/manga/", "/online/", 1)))
		.unwrap_or_default();
	format!("{BASE_URL}{key}")
}

pub fn hq_thumbnail(url: &str) -> String {
	let is_blurred = url.contains("/manganew_thumbs_blur/");
	let mut result = String::from(url);

	if let Some(start) = result.find("/manganew_thumbs")
		&& let Some(end) = result[start + 1..].find('/').map(|index| start + 1 + index)
	{
		result.replace_range(start..end, "/showfull_retina/manga");
	}

	result = result.replace("_xxl.hentaichan.live", "_hentaichan.ru");
	if is_blurred {
		result.push('#');
	}
	result
}

pub fn browse_url(
	page: i32,
	sort_index: usize,
	ascending: bool,
	included_genres: &[String],
	excluded_genres: &[String],
) -> String {
	let offset = 20 * (page - 1);
	if !included_genres.is_empty() || !excluded_genres.is_empty() {
		let mut genres = String::new();
		for genre in included_genres {
			genres.push_str(genre);
			genres.push('+');
		}
		for genre in excluded_genres {
			genres.push('-');
			genres.push_str(genre);
			genres.push('+');
		}
		genres.pop();

		let order = match (sort_index, ascending) {
			(0, true) => "&n=dateasc",
			(0, false) => "",
			(1, true) => "&n=favasc",
			(1, false) => "&n=favdesc",
			(2, true) => "&n=abcdesc",
			(2, false) => "&n=abcasc",
			_ => "&n=favdesc",
		};
		format!("{BASE_URL}/tags/{genres}&sort=manga{order}?offset={offset}")
	} else {
		let path = match (sort_index, ascending) {
			(0, true) => "manga/new&n=dateasc",
			(0, false) => "manga/new",
			(1, true) => "manga/new&n=favasc",
			(1, false) => "mostfavorites&sort=manga",
			(2, true) => "manga/new&n=abcdesc",
			(2, false) => "manga/new&n=abcasc",
			_ => "mostfavorites&sort=manga",
		};
		format!("{BASE_URL}/{path}?offset={offset}")
	}
}

pub fn page_urls(html: &str) -> Option<Vec<String>> {
	let key = "fullimg\":";
	let key_index = html.find(key)?;
	let after_key = key_index + key.len();
	let start = after_key + html[after_key..].find('[')? + 1;
	let end = start + html[start..].find(']')?;

	Some(
		html[start..end]
			.split(',')
			.filter_map(|url| {
				let url = url.trim().trim_matches(['"', '\'']);
				(!url.is_empty()).then(|| url.into())
			})
			.collect(),
	)
}

pub fn chapter_number(title: &str) -> Option<f32> {
	let title = title.to_lowercase();
	for marker in ["глава", "часть"] {
		if let Some(index) = title.find(marker) {
			let number: String = title[index + marker.len()..]
				.chars()
				.skip_while(|c| c.is_whitespace())
				.take_while(|c| c.is_ascii_digit() || *c == '.')
				.collect();
			return number.parse().ok();
		}
	}
	None
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn manga_key_accepts_manga_urls_and_rejects_other_sections() {
		assert_eq!(
			manga_key("https://x9.h-chan.me/manga/55582-mama-gyaru-anna-san.html?cacheId=1")
				.as_deref(),
			Some("/manga/55582-mama-gyaru-anna-san.html")
		);
		assert_eq!(
			manga_key("/manga/55582-mama-gyaru-anna-san.html").as_deref(),
			Some("/manga/55582-mama-gyaru-anna-san.html")
		);
		assert_eq!(
			manga_key("https://x9.h-chan.me/games/28330-roundscape.html"),
			None
		);
	}

	#[aidoku_test]
	fn reader_url_maps_manga_pages_to_online_reader() {
		assert_eq!(
			reader_url("/manga/55582-mama-gyaru-anna-san.html"),
			"https://xxl.hentaichan.live/online/55582-mama-gyaru-anna-san.html"
		);
		assert_eq!(
			reader_url("/online/55582-mama-gyaru-anna-san.html"),
			"https://xxl.hentaichan.live/online/55582-mama-gyaru-anna-san.html"
		);
	}

	#[aidoku_test]
	fn hq_thumbnail_rewrites_thumb_paths_and_marks_blurred_sources() {
		assert_eq!(
			hq_thumbnail("https://img4.imgschan.xyz/manganew_thumbs/a/1784219215_anna-san/01.jpg"),
			"https://img4.imgschan.xyz/showfull_retina/manga/a/1784219215_anna-san/01.jpg"
		);
		assert_eq!(
			hq_thumbnail(
				"https://img4.imgschan.xyz/manganew_thumbs_blur/a/1784219215_xxl.hentaichan.live/01.jpg"
			),
			"https://img4.imgschan.xyz/showfull_retina/manga/a/1784219215_hentaichan.ru/01.jpg#"
		);
	}

	#[aidoku_test]
	fn browse_url_uses_popularity_by_default() {
		assert_eq!(
			browse_url(2, 1, false, &[], &[]),
			"https://xxl.hentaichan.live/mostfavorites&sort=manga?offset=20"
		);
	}

	#[aidoku_test]
	fn browse_url_combines_genres_exclusions_and_sorting() {
		let included = Vec::from([String::from("x-ray")]);
		let excluded = Vec::from([String::from("фемдом")]);
		assert_eq!(
			browse_url(2, 2, false, &included, &excluded),
			"https://xxl.hentaichan.live/tags/x-ray+-фемдом&sort=manga&n=abcasc?offset=20"
		);
	}

	#[aidoku_test]
	fn page_urls_reads_current_and_legacy_fullimg_arrays() {
		assert_eq!(
			page_urls(r#"var data = { "fullimg": ['https://img/01.jpg', 'https://img/02.jpg'] }"#),
			Some(Vec::from([
				String::from("https://img/01.jpg"),
				String::from("https://img/02.jpg")
			]))
		);
		assert_eq!(
			page_urls(r#"var data = { "fullimg":["https://img/01.jpg","https://img/02.jpg",] }"#),
			Some(Vec::from([
				String::from("https://img/01.jpg"),
				String::from("https://img/02.jpg")
			]))
		);
		assert_eq!(page_urls("var data = {}"), None);
	}

	#[aidoku_test]
	fn chapter_number_reads_russian_chapter_markers() {
		assert_eq!(chapter_number("Ecstasy Collection - глава 12"), Some(12.0));
		assert_eq!(
			chapter_number("Oideyo! Mizuryu Kei Land - часть 2.5"),
			Some(2.5)
		);
		assert_eq!(chapter_number("Title - глава  12"), Some(12.0));
		assert_eq!(chapter_number("Title - часть\t2"), Some(2.0));
		assert_eq!(chapter_number("Single chapter"), None);
	}
}
