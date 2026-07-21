#![no_std]

extern crate alloc;

pub mod helpers;

use aidoku::{
	Chapter, ContentRating, FilterValue, ImageRequestProvider, Listing, ListingProvider, Manga,
	MangaPageResult, Page, PageContent, PageContext, Result, Source,
	alloc::{String, Vec, format, vec},
	imports::{
		html::{Document, Element},
		net::Request,
		std::send_partial_result,
	},
	prelude::*,
};

use helpers as h;

#[allow(dead_code)]
const BASE_URL: &str = "https://acomics.ru";
#[allow(dead_code)]
const MANGA_SELECTOR: &str = "section.serial-card";
#[allow(dead_code)]
const NEXT_PAGE_SELECTOR: &str = "a.infinite-scroll";

#[allow(dead_code)]
struct AComics;

#[allow(dead_code)]
fn request(url: String) -> Result<Request> {
	Ok(Request::get(url)?
		.header("Referer", "https://acomics.ru/")
		.header("Cookie", "ageRestrict=17"))
}

#[allow(dead_code)]
fn get_html(url: String) -> Result<Document> {
	let response = request(url)?.send()?;
	if response.status_code() >= 400 {
		bail!("AComics HTTP error {}", response.status_code());
	}
	Ok(response.get_html()?)
}

#[allow(dead_code)]
fn parse_manga_list(url: String) -> Result<MangaPageResult> {
	let html = get_html(url)?;
	let entries: Vec<Manga> = html
		.select(MANGA_SELECTOR)
		.map(|els| els.filter_map(|el| manga_from_element(&el)).collect())
		.unwrap_or_default();
	let has_next = html.select_first(NEXT_PAGE_SELECTOR).is_some();

	Ok(MangaPageResult {
		entries,
		has_next_page: has_next,
	})
}

#[allow(dead_code)]
fn manga_from_element(element: &Element) -> Option<Manga> {
	let title_el = element.select_first("h2.title > a")?;
	let href = title_el.attr("href")?;
	let key = h::manga_key(&href)?;
	let details = h::details_url(&key);
	let title = title_el.text()?;
	let cover = element
		.select_first("a.cover img")
		.and_then(|img| img.attr("abs:data-real-src"));

	Some(Manga {
		key,
		title,
		cover,
		url: Some(details),
		content_rating: ContentRating::Safe,
		..Default::default()
	})
}

#[allow(dead_code)]
fn parse_details(html: &Document, manga: &mut Manga) {
	if let Some(title_el) = html.select_first("article.common-article h1") {
		manga.title = title_el.text().unwrap_or_default();
	}

	manga.tags = Some(
		html.select("article.common-article p.serial-about-badges a.category")
			.map(|els| {
				els.filter_map(|el| el.text())
					.map(|t| t.trim().into())
					.collect::<Vec<String>>()
			})
			.unwrap_or_default(),
	);

	manga.authors = html
		.select_first(
			"article.common-article p.serial-about-authors a, article.common-article p:contains(Автор оригинала)",
		)
		.and_then(|el| el.text())
		.map(|t| vec![t.trim().into()]);

	manga.description = html
		.select_first("article.common-article section.serial-about-text")
		.and_then(|el| el.text());

	if let Some(cover_el) = html.select_first("meta[property=\"og:image\"]") {
		manga.cover = cover_el.attr("abs:content");
	}
}

#[allow(dead_code)]
fn chapter_count(html: &Document) -> Option<i32> {
	let text = html
		.select_first("p:has(b:contains(Количество выпусков:))")?
		.own_text()?;
	text.trim().parse().ok()
}

#[allow(dead_code)]
fn build_filter_params(filters: &[FilterValue], page: i32) -> Vec<(String, String)> {
	let is_first = page == 1;
	let mut params: Vec<(String, String)> = Vec::new();

	for filter in filters {
		match filter {
			FilterValue::MultiSelect { id, included, .. } if id == "categories" => {
				let ids: Vec<&str> = included.iter().filter_map(|n| h::category_id(n)).collect();
				for (i, val) in ids.iter().enumerate() {
					let key = if is_first {
						String::from("categories[]")
					} else {
						format!("categories[{i}]")
					};
					params.push((key, (*val).into()));
				}
			}
			FilterValue::Select { id, value, .. } if id == "type" => {
				let val = match value.as_str() {
					"Оригинальный" => "orig",
					"Перевод" => "trans",
					_ => "0",
				};
				params.push(("type".into(), val.into()));
			}
			FilterValue::Select { id, value, .. } if id == "publication" => {
				let val = match value.as_str() {
					"Завершенный" => "no",
					"Продолжающийся" => "yes",
					_ => "0",
				};
				params.push(("updatable".into(), val.into()));
			}
			FilterValue::Select { id, value, .. } if id == "subscription" => {
				let val = match value.as_str() {
					"В моей ленте" => "yes",
					"Кроме моей ленты" => "no",
					_ => "0",
				};
				params.push(("subscribe".into(), val.into()));
			}
			FilterValue::Text { id, value, .. } if id == "min_pages" => {
				params.push(("issue_count".into(), value.clone()));
			}
			FilterValue::Sort { id, index, .. } if id == "sort" => {
				let val = match index {
					0 => "last_update",
					2 => "issue_count",
					3 => "serial_name",
					_ => "subscr_count",
				};
				params.push(("sort".into(), val.into()));
			}
			_ => {}
		}
	}

	params
}

#[allow(dead_code)]
fn add_ratings(params: &mut Vec<(String, String)>, filters: &[FilterValue], page: i32) {
	let is_first = page == 1;
	let mut rating_ids: Vec<&str> = Vec::new();

	for filter in filters {
		if let FilterValue::MultiSelect { id, included, .. } = filter
			&& id == "ratings"
		{
			rating_ids = included.iter().filter_map(|n| h::rating_id(n)).collect();
		}
	}

	// Default: all ratings selected
	if rating_ids.is_empty() {
		rating_ids = vec!["1", "2", "3", "4", "5", "6"];
	}

	for (i, val) in rating_ids.iter().enumerate() {
		let key = if is_first {
			String::from("ratings[]")
		} else {
			format!("ratings[{i}]")
		};
		params.push((key, (*val).into()));
	}
}

impl Source for AComics {
	fn new() -> Self {
		AComics
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		if let Some(query) = query.filter(|q| !q.trim().is_empty()) {
			return parse_manga_list(h::search_url(&query));
		}

		let mut params = build_filter_params(&filters, page);
		add_ratings(&mut params, &filters, page);
		parse_manga_list(h::browse_url(page, &params))
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

		let html = get_html(h::details_url(&manga.key))?;
		if needs_details {
			parse_details(&html, &mut manga);
		}
		if needs_chapters {
			let count = chapter_count(&html).unwrap_or(0);
			if needs_details && count > 0 {
				send_partial_result(&manga);
			}
			let chapters: Vec<Chapter> = (1..=count)
				.rev()
				.map(|issue| {
					let key = format!("{}/{}", manga.key, issue);
					Chapter {
						key,
						chapter_number: Some(issue as f32),
						title: Some(format!("Выпуск {issue}")),
						..Default::default()
					}
				})
				.collect();
			manga.chapters = Some(chapters);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let url = format!("{BASE_URL}{}", chapter.key);
		let html = get_html(url)?;
		let img = html
			.select_first("img.issue")
			.and_then(|el| el.attr("abs:src"))
			.ok_or_else(|| error!("Page image not found"))?;

		Ok(vec![Page {
			content: PageContent::url(img),
			..Default::default()
		}])
	}
}

impl ListingProvider for AComics {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let sort = match listing.id.as_str() {
			"latest" => "last_update",
			_ => "subscr_count",
		};

		let is_first = page == 1;
		let mut params: Vec<(String, String)> = Vec::new();
		let ratings = ["1", "2", "3", "4", "5", "6"];
		for (i, r) in ratings.iter().enumerate() {
			let key = if is_first {
				String::from("ratings[]")
			} else {
				format!("ratings[{i}]")
			};
			params.push((key, (*r).into()));
		}
		params.push(("sort".into(), sort.into()));

		parse_manga_list(h::browse_url(page, &params))
	}
}

impl ImageRequestProvider for AComics {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		request(url)
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use aidoku::imports::html::Html;
	use aidoku_test::aidoku_test;

	#[aidoku_test]
	fn parses_manga_list_cards() {
		let html = Html::parse_with_url(
			r#"<section class="serial-card"><a href="/~LwHG" class="cover" title="Читать комикс LwHG онлайн"><img src="/static/img/tail-spin.svg" data-real-src="/upload/cover.jpg" width="160" height="90" alt="Обложка"></a><h2 class="title"><a href="/~LwHG" title="Читать комикс LwHG онлайн">LwHG</a></h2><p class="about">Описание</p></section><section class="serial-card"><a href="/~test" class="cover"><img data-real-src="/upload/test.jpg"></a><h2 class="title"><a href="/~test">Test Comic</a></h2></section><a class="infinite-scroll" href="/comics?skip=10">Показать еще</a>"#,
			BASE_URL,
		)
		.unwrap();

		let entries: Vec<Manga> = html
			.select(MANGA_SELECTOR)
			.map(|els| els.filter_map(|el| manga_from_element(&el)).collect())
			.unwrap_or_default();
		let has_next = html.select_first(NEXT_PAGE_SELECTOR).is_some();

		assert_eq!(entries.len(), 2);
		assert!(has_next);

		let first = &entries[0];
		assert_eq!(first.key, "/~LwHG");
		assert_eq!(first.title, "LwHG");
		assert_eq!(
			first.cover,
			Some("https://acomics.ru/upload/cover.jpg".into())
		);
		assert_eq!(first.url, Some("https://acomics.ru/~LwHG/about".into()));

		let second = &entries[1];
		assert_eq!(second.key, "/~test");
		assert_eq!(second.title, "Test Comic");
	}

	#[aidoku_test]
	fn returns_empty_for_no_cards() {
		let html = Html::parse_with_url("<div>nothing</div>", BASE_URL).unwrap();
		let entries: Vec<Manga> = html
			.select(MANGA_SELECTOR)
			.map(|els| els.filter_map(|el| manga_from_element(&el)).collect())
			.unwrap_or_default();
		assert!(entries.is_empty());
	}

	#[aidoku_test]
	fn parses_details_page() {
		let html = Html::parse_with_url(
			r#"<article class="common-article"><h1>Test Comic / Русский</h1><p class="serial-about-badges"><a class="category">Комедия</a><a class="category">Драма</a></p><p class="serial-about-authors"><a>Иван Иванов</a></p><section class="serial-about-text">Длинное описание</section></article><meta property="og:image" content="/upload/og.jpg">"#,
			BASE_URL,
		)
		.unwrap();

		let mut manga = Manga {
			key: "/~test".into(),
			..Default::default()
		};
		parse_details(&html, &mut manga);

		assert_eq!(manga.title, "Test Comic / Русский");
		assert_eq!(manga.tags, Some(vec!["Комедия".into(), "Драма".into()]));
		assert_eq!(manga.authors, Some(vec!["Иван Иванов".into()]));
		assert_eq!(manga.description, Some("Длинное описание".into()));
		assert_eq!(manga.cover, Some("https://acomics.ru/upload/og.jpg".into()));
	}

	#[aidoku_test]
	fn chapter_count_parses() {
		// .own_text() only reads text directly in <p>, excluding <b> child
		let html =
			Html::parse_with_url(r#"<p><b>Количество выпусков:</b> 42</p>"#, BASE_URL).unwrap();
		assert_eq!(chapter_count(&html), Some(42));
	}
}

register_source!(AComics, ListingProvider, ImageRequestProvider);
