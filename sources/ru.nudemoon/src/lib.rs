#![no_std]

extern crate alloc;

mod helpers;

use aidoku::{
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, MangaStatus, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec, vec},
	imports::{
		html::{Document, Element},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
};

const BASE_URL: &str = "https://nude-moon.org";
const COOKIE: &str = "NMfYa=1; nm_mobile=1; Domain=nude-moon.org";
const MANGA_SELECTOR: &str = "table.news_pic2";
const NEXT_PAGE_SELECTOR: &str = "a.small:contains(>)";

struct Nudemoon;

impl Nudemoon {
	fn request(url: String) -> Result<Request> {
		Ok(Request::get(url)?
			.header("Referer", &format!("{BASE_URL}/"))
			.header("Cookie", COOKIE))
	}

	fn get_html(url: String) -> Result<Document> {
		let response = Self::request(url)?.send()?;
		let status = response.status_code();
		// Aidoku handles Cloudflare in a WebView and stores cf_clearance in shared cookies.
		// If it still returns a challenge, surface an actionable error instead of parsing it.
		if Self::is_cloudflare_challenge(status, response.get_header("cf-mitigated")) {
			bail!("Cloudflare verification required. Complete the WebView challenge and try again");
		}
		if status >= 400 {
			bail!("Nude-Moon HTTP error {status}");
		}
		Ok(response.get_html()?)
	}

	fn is_cloudflare_challenge(status: i32, cf_mitigated: Option<String>) -> bool {
		matches!(status, 403 | 503) && cf_mitigated.is_some_and(|value| value == "challenge")
	}

	fn parse_manga_list(url: String) -> Result<MangaPageResult> {
		let html = Self::get_html(url)?;
		let entries = html
			.select(MANGA_SELECTOR)
			.map(|els| els.filter_map(|el| Self::manga_from_element(&el)).collect())
			.unwrap_or_default();
		let has_next_page = html.select_first(NEXT_PAGE_SELECTOR).is_some();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn manga_from_element(element: &Element) -> Option<Manga> {
		let link = element.select_first("a:has(h2)")?;
		let href = link.attr("abs:href")?;
		let key = helpers::url_key(&href)?;
		let title = helpers::clean_title(&link.text()?);
		if title.is_empty() {
			return None;
		}
		let cover = element
			.select_first("a img")
			.and_then(|img| img.attr("abs:src"));
		let url = format!("{BASE_URL}{key}");

		Some(Manga {
			key,
			title,
			cover,
			url: Some(url),
			content_rating: ContentRating::NSFW,
			..Default::default()
		})
	}

	fn parse_details(html: &Document, manga: &mut Manga) {
		if let Some(title) = html.select_first("h1").and_then(|el| el.text()) {
			manga.title = helpers::clean_title(&title);
		}

		if let Some(info) = html.select_first(MANGA_SELECTOR) {
			manga.authors = info
				.select_first("a[href*=mangaka]")
				.and_then(|el| el.text())
				.map(|author| vec![author]);
			manga.tags = info.select("div.tag-links a").map(|els| {
				els.filter_map(|el| el.text())
					.filter(|tag| !tag.trim().is_empty())
					.collect()
			});
		}

		manga.description = html
			.select_first(".description")
			.and_then(|el| el.text())
			.map(|description| description.trim().into());
		if let Some(cover) = html
			.select_first("meta[property='og:image']")
			.and_then(|el| el.attr("content"))
			.map(|cover| Self::absolute_url(&cover))
		{
			manga.cover = Some(cover);
		}
		manga.status = MangaStatus::Unknown;
		manga.content_rating = ContentRating::NSFW;
		manga.url = Some(format!("{BASE_URL}{}", manga.key));
	}

	fn fetch_chapters(&self, manga: &Manga, details: &Document) -> Result<Vec<Chapter>> {
		let Some(all_chapters_link) = details.select_first("td.button a:contains(Все главы)")
		else {
			return Ok(vec![Self::single_chapter(details, manga)]);
		};
		let Some(mut page_url) = all_chapters_link.attr("abs:href") else {
			return Ok(vec![Self::single_chapter(details, manga)]);
		};

		let mut chapters: Vec<Chapter> = Vec::new();
		loop {
			let html = Self::get_html(page_url)?;
			let mut page_chapters = Self::chapters_from_document(&html);
			if page_chapters.is_empty() {
				if chapters.is_empty() {
					chapters.push(Self::single_chapter(details, manga));
				}
				break;
			}
			chapters.append(&mut page_chapters);

			let Some(next_url) = html
				.select_first(NEXT_PAGE_SELECTOR)
				.and_then(|el| el.attr("abs:href"))
			else {
				break;
			};
			page_url = next_url;
		}

		Ok(chapters)
	}

	fn chapters_from_document(html: &Document) -> Vec<Chapter> {
		html.select(MANGA_SELECTOR)
			.map(|els| {
				els.filter_map(|el| {
					let link = el.select_first("tr[valign=top] a:has(h2)")?;
					let title = link.select_first("h2").and_then(|el| el.text())?;
					let href = link.attr("abs:href")?;
					let key = helpers::url_key(&href)?;
					let info = el.select_first("tr[valign=top] td[align=left]");

					let url = format!("{BASE_URL}{key}");

					Some(Chapter {
						key,
						title: Some(title.clone()),
						chapter_number: helpers::chapter_number(&title),
						date_uploaded: info
							.as_ref()
							.and_then(Self::date_text)
							.and_then(|date| Self::parse_date(&date)),
						scanlators: info
							.as_ref()
							.and_then(|el| el.select_first("a[href*=perevod]"))
							.and_then(|el| el.text())
							.map(|scanlator| vec![scanlator]),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default()
	}

	fn single_chapter(details: &Document, manga: &Manga) -> Chapter {
		let title = details
			.select_first("table td.bg_style1 h1")
			.and_then(|el| el.text())
			.map(|title| format!("{title} Сингл"))
			.unwrap_or_else(|| format!("{} Сингл", manga.title));

		Chapter {
			key: manga.key.clone(),
			title: Some(title),
			chapter_number: Some(0.0),
			date_uploaded: details
				.select_first(MANGA_SELECTOR)
				.and_then(|el| Self::date_text(&el))
				.and_then(|date| Self::parse_date(&date)),
			scanlators: details
				.select_first(MANGA_SELECTOR)
				.and_then(|el| el.select_first("a[href*=perevod]"))
				.and_then(|el| el.text())
				.map(|scanlator| vec![scanlator]),
			url: manga.url.clone(),
			..Default::default()
		}
	}

	fn date_text(element: &Element) -> Option<String> {
		element.select("span.small2").and_then(|els| {
			els.filter_map(|el| el.text())
				.find(|text| Self::looks_like_date(text))
		})
	}

	fn looks_like_date(text: &str) -> bool {
		let mut parts = text.split_whitespace();
		let day = parts.next().and_then(|part| part.parse::<u32>().ok());
		let month = parts.next();
		let year = parts.next().and_then(|part| part.parse::<u32>().ok());
		matches!(day, Some(1..=31)) && month.is_some() && matches!(year, Some(1900..=2099))
	}

	fn pages_from_document(html: &Document) -> Vec<Page> {
		html.select("img[loading=\"lazy\"][title]")
			.map(|els| {
				els.filter_map(|img| {
					let has_title = img
						.attr("title")
						.is_some_and(|title| !title.trim().is_empty());
					if !has_title {
						return None;
					}
					let url = img.attr("abs:data-src")?;
					Some(Page {
						content: PageContent::url(url),
						..Default::default()
					})
				})
				.collect::<Vec<_>>()
			})
			.unwrap_or_default()
	}

	fn parse_date(text: &str) -> Option<i64> {
		let mut parts = text.split_whitespace();
		let day = parts.next()?.parse::<u32>().ok()?;
		let month = match parts.next()?.to_lowercase().as_str() {
			"января" | "январь" => 1,
			"февраля" | "февраль" => 2,
			"марта" | "март" => 3,
			"апреля" | "апрель" => 4,
			"мая" | "май" => 5,
			"июня" | "июнь" => 6,
			"июля" | "июль" => 7,
			"августа" | "август" => 8,
			"сентября" | "сентябрь" => 9,
			"октября" | "октябрь" => 10,
			"ноября" | "ноябрь" => 11,
			"декабря" | "декабрь" => 12,
			_ => return None,
		};
		let year = parts.next()?.parse::<i64>().ok()?;
		if day == 0 || day > Self::days_in_month(year, month) {
			return None;
		}
		Some(Self::days_from_civil(year, month, day) * 86_400)
	}

	fn absolute_url(url: &str) -> String {
		if url.starts_with("http://") || url.starts_with("https://") {
			url.into()
		} else if url.starts_with('/') {
			format!("{BASE_URL}{url}")
		} else {
			format!("{BASE_URL}/{url}")
		}
	}

	fn days_in_month(year: i64, month: u32) -> u32 {
		match month {
			1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
			4 | 6 | 9 | 11 => 30,
			2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
			2 => 28,
			_ => 0,
		}
	}

	fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
		let year = if month <= 2 { year - 1 } else { year };
		let era = if year >= 0 { year } else { year - 399 } / 400;
		let year_of_era = year - era * 400;
		let month_prime = (month as i64 + 9) % 12;
		let day_of_year = (153 * month_prime + 2) / 5 + day as i64 - 1;
		let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
		era * 146_097 + day_of_era - 719_468
	}
}

impl Source for Nudemoon {
	fn new() -> Self {
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = if let Some(query) = query.filter(|query| !query.trim().is_empty()) {
			helpers::search_url(&query, page)
		} else {
			let mut sort_index = 1usize;
			let mut genres: Vec<String> = Vec::new();
			for filter in filters {
				match filter {
					FilterValue::Sort { id, index, .. } if id == "sort" => {
						sort_index = index as usize;
					}
					FilterValue::MultiSelect {
						id,
						included,
						excluded: _,
					} if id == "genre" => {
						genres = included;
					}
					_ => {}
				}
			}
			helpers::browse_url(page, sort_index, &genres)
		};

		Self::parse_manga_list(url)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		if !needs_details && !needs_chapters {
			return Ok(manga);
		}

		let html = Self::get_html(format!("{BASE_URL}{}", manga.key))?;
		if needs_details {
			Self::parse_details(&html, &mut manga);
		}
		if needs_chapters {
			if needs_details
				&& html
					.select_first("td.button a:contains(Все главы)")
					.is_some()
			{
				send_partial_result(&manga);
			}
			manga.chapters = Some(self.fetch_chapters(&manga, &html)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = Self::get_html(format!("{BASE_URL}{}", chapter.key))?;
		let pages = Self::pages_from_document(&html);

		if pages.is_empty() {
			bail!("Pages not found. Authorization may be required");
		}
		Ok(pages)
	}
}

impl ListingProvider for Nudemoon {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort_index = match listing.id.as_str() {
			"latest" => 0,
			"popular" => 1,
			_ => return Ok(MangaPageResult::default()),
		};
		Self::parse_manga_list(helpers::browse_url(page, sort_index, &[]))
	}
}

impl ImageRequestProvider for Nudemoon {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Self::request(url)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::imports::html::Html;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn parses_manga_list_entry() {
		let html = Html::parse_with_url(
			r#"<table class="news_pic2"><tr><td>
			<a href="/manga/123-example"><img src="/covers/example.jpg"></a>
			<a href="/manga/123-example"><h2>Example Title / Русский №2</h2></a>
			</td></tr></table>"#,
			BASE_URL,
		)
		.unwrap();
		let element = html.select_first(MANGA_SELECTOR).unwrap();
		let manga = Nudemoon::manga_from_element(&element).unwrap();

		assert_eq!(manga.key, "/manga/123-example");
		assert_eq!(manga.title, "Example Title");
		assert_eq!(
			manga.cover,
			Some(String::from("https://nude-moon.org/covers/example.jpg"))
		);
		assert_eq!(manga.content_rating, ContentRating::NSFW);
	}

	#[aidoku_test]
	fn parses_chapter_row() {
		let html = Html::parse_with_url(
			r#"<table class="news_pic2"><tr valign="top"><td align="left">
			<a href="/reader/123-example"><h2>Example Chapter №12.5</h2></a>
			<a href="/perevod/team">Team</a>
			<span class="small2">12 Май 2024</span>
			</td></tr></table>"#,
			BASE_URL,
		)
		.unwrap();
		let chapters = Nudemoon::chapters_from_document(&html);

		assert_eq!(chapters.len(), 1);
		let chapter = &chapters[0];
		assert_eq!(chapter.key, "/reader/123-example");
		assert_eq!(chapter.chapter_number, Some(12.5));
		assert_eq!(chapter.scanlators, Some(Vec::from([String::from("Team")])));
		assert_eq!(chapter.date_uploaded, Some(1_715_472_000));
	}

	#[aidoku_test]
	fn parses_manga_details() {
		let html = Html::parse_with_url(
			r#"<html><head><meta property="og:image" content="/cover.jpg"></head><body>
			<h1>Example Title / Русский №2</h1>
			<table class="news_pic2"><tr><td>
			<a href="/mangaka/author">Author</a>
			<div class="tag-links"><a href="/tags/ahegao">ahegao</a><a href="/tags/x-ray">x-ray</a></div>
			</td></tr></table>
			<div class="description"> Example description </div>
			</body></html>"#,
			BASE_URL,
		)
		.unwrap();
		let mut manga = Manga {
			key: "/manga/123-example".into(),
			title: "Old title".into(),
			..Default::default()
		};
		Nudemoon::parse_details(&html, &mut manga);

		assert_eq!(manga.title, "Example Title");
		assert_eq!(manga.authors, Some(Vec::from([String::from("Author")])));
		assert_eq!(
			manga.tags,
			Some(Vec::from([String::from("ahegao"), String::from("x-ray")]))
		);
		assert_eq!(manga.description, Some(String::from("Example description")));
		assert_eq!(
			manga.cover,
			Some(String::from("https://nude-moon.org/cover.jpg"))
		);
	}

	#[aidoku_test]
	fn detects_cloudflare_challenge() {
		assert!(Nudemoon::is_cloudflare_challenge(
			403,
			Some(String::from("challenge"))
		));
		assert!(Nudemoon::is_cloudflare_challenge(
			503,
			Some(String::from("challenge"))
		));
		assert!(!Nudemoon::is_cloudflare_challenge(
			403,
			Some(String::from("managed"))
		));
		assert!(!Nudemoon::is_cloudflare_challenge(200, None));
	}

	#[aidoku_test]
	fn parses_reader_pages_and_skips_empty_titles() {
		let html = Html::parse_with_url(
			r#"<div>
			<img loading="lazy" title="1" data-src="/pages/1.jpg">
			<img loading="lazy" title="" data-src="/pages/phantom.jpg">
			</div>"#,
			BASE_URL,
		)
		.unwrap();
		let pages = Nudemoon::pages_from_document(&html);

		assert_eq!(pages.len(), 1);
	}
}

register_source!(Nudemoon, ListingProvider, ImageRequestProvider);
