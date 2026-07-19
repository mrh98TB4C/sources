#![no_std]

extern crate alloc;

mod helpers;

use aidoku::{
	Chapter, ContentRating, DeepLinkHandler, DeepLinkResult, FilterValue, ImageRequestProvider,
	Listing, ListingProvider, Manga, MangaPageResult, MangaStatus, Page, PageContent, PageContext,
	Result, Source, Viewer,
	alloc::{String, Vec, string::ToString},
	helpers::uri::QueryParameters,
	imports::{
		html::{Document, Element},
		net::{Request, TimeUnit, set_rate_limit},
		std::{parse_date_with_options, send_partial_result},
	},
	prelude::*,
};

const BASE_URL: &str = "https://xxl.hentaichan.live";

struct HenChan;

impl HenChan {
	fn parse_manga_list(url: String) -> Result<MangaPageResult> {
		let html = Request::get(url)?.html()?;
		let entries = html
			.select(".content_row")
			.map(|els| els.filter_map(|el| Self::manga_from_element(&el)).collect())
			.unwrap_or_default();
		let has_next_page = html.select_first("a:contains(Вперед)").is_some()
			|| html.select_first("a:contains(Далее)").is_some();

		Ok(MangaPageResult {
			entries,
			has_next_page,
		})
	}

	fn manga_from_element(element: &Element) -> Option<Manga> {
		let link = element.select_first("h2 a")?;
		let href = link.attr("abs:href")?;
		let key = helpers::manga_key(&href)?;
		let title = element
			.attr("title")
			.or_else(|| link.attr("title"))
			.or_else(|| link.text())?;
		let cover = element
			.select_first("img")
			.and_then(|img| img.attr("abs:src"))
			.map(|url| helpers::hq_thumbnail(&url));
		let url = format!("{BASE_URL}{key}");

		Some(Manga {
			key,
			title,
			cover,
			url: Some(url),
			..Default::default()
		})
	}

	fn parse_details(&self, html: &Document, manga: &mut Manga, url: String) {
		if let Some(title) = html.select_first("a.title_top_a").and_then(|el| el.text()) {
			manga.title = title;
		}

		let was_blurred = manga
			.cover
			.as_deref()
			.is_some_and(|cover| cover.ends_with('#'));
		if let Some(mut cover) = html
			.select_first("img#cover")
			.and_then(|img| img.attr("abs:src"))
			.map(|url| helpers::hq_thumbnail(&url))
		{
			if was_blurred && !cover.ends_with('#') {
				cover.push('#');
			}
			manga.cover = Some(cover);
		}

		manga.authors = html
			.select("#info_wrap .row:has(.item:contains(Автор)) .item2 a")
			.map(|els| {
				els.filter_map(|el| el.text())
					.filter(|author: &String| !author.trim().is_empty())
					.collect()
			});
		manga.description = html
			.select_first("div#description")
			.and_then(|el| el.text())
			.map(|description| description.trim().to_string());
		manga.tags = html
			.select(".sidetags ul li a:last-child")
			.map(|els| els.filter_map(|el| el.text()).collect());

		let info_text = html
			.select_first("#info_wrap")
			.and_then(|el| el.text())
			.unwrap_or_default();
		manga.status = if info_text.contains("перевод завершен") {
			MangaStatus::Completed
		} else if info_text.contains("перевод продолжается") {
			MangaStatus::Ongoing
		} else {
			MangaStatus::Unknown
		};
		manga.content_rating = ContentRating::NSFW;
		manga.viewer = Viewer::RightToLeft;
		manga.url = Some(url);
	}

	fn scanlators_from_document(html: &Document) -> Option<Vec<String>> {
		html.select("#info_wrap .row:has(.item:contains(Переводчик)) .item2 a")
			.map(|els| {
				els.filter_map(|el| el.text())
					.filter(|scanlator: &String| !scanlator.trim().is_empty())
					.collect()
			})
	}

	fn fetch_chapters(&self, manga: &Manga, html: &Document) -> Result<Vec<Chapter>> {
		let scanlators = Self::scanlators_from_document(html);
		let is_single_chapter = manga
			.cover
			.as_deref()
			.is_some_and(|cover| cover.ends_with('#'));

		if is_single_chapter {
			let title = html
				.select_first("a.title_top_a")
				.and_then(|el| el.text())
				.unwrap_or_else(|| manga.title.clone());
			let date_uploaded = html
				.select_first(".row4_right b")
				.and_then(|el| el.text())
				.and_then(|date| parse_date_with_options(&date, "d MMMM yyyy", "ru_RU", "current"));

			return Ok(Vec::from([Chapter {
				key: manga.key.clone(),
				title: Some(title),
				chapter_number: Some(1.0),
				date_uploaded,
				scanlators,
				url: Some(format!("{BASE_URL}{}", manga.key)),
				..Default::default()
			}]));
		}

		let related_key = manga.key.replacen("/manga/", "/related/", 1);
		let mut html = Request::get(format!("{BASE_URL}{related_key}"))?.html()?;
		let mut chapters = Self::related_chapters_from_document(&html);

		while let Some(next_url) = html
			.select_first("div#pagination_related a:contains(Вперед)")
			.and_then(|el| el.attr("abs:href"))
		{
			html = Request::get(next_url)?.html()?;
			chapters.extend(Self::related_chapters_from_document(&html));
		}

		if chapters.is_empty() {
			chapters.push(Chapter {
				key: manga.key.clone(),
				title: Some(manga.title.clone()),
				chapter_number: Some(1.0),
				scanlators,
				url: Some(format!("{BASE_URL}{}", manga.key)),
				..Default::default()
			});
		} else {
			chapters.reverse();
		}

		Ok(chapters)
	}

	fn related_chapters_from_document(html: &Document) -> Vec<Chapter> {
		html.select(".related")
			.map(|els| {
				els.filter_map(|el| {
					let link = el.select_first("h2 a")?;
					let href = link.attr("abs:href")?;
					let key = helpers::manga_key(&href)?;
					let title = link.attr("title").or_else(|| link.text())?;
					let url = format!("{BASE_URL}{key}");

					Some(Chapter {
						key,
						title: Some(title.clone()),
						chapter_number: helpers::chapter_number(&title),
						url: Some(url),
						..Default::default()
					})
				})
				.collect()
			})
			.unwrap_or_default()
	}
}

impl Source for HenChan {
	fn new() -> Self {
		set_rate_limit(2, 1, TimeUnit::Seconds);
		Self
	}

	fn get_search_manga_list(
		&self,
		query: Option<String>,
		page: i32,
		filters: Vec<FilterValue>,
	) -> Result<MangaPageResult> {
		let url = if let Some(query) = query.filter(|query| !query.is_empty()) {
			let mut qs = QueryParameters::new();
			qs.push("do", Some("search"));
			qs.push("subaction", Some("search"));
			qs.push("story", Some(&query));
			qs.push("search_start", Some(&page.to_string()));
			format!("{BASE_URL}/?{qs}")
		} else {
			let mut sort_index = 1;
			let mut ascending = false;
			let mut included_genres = Vec::new();
			let mut excluded_genres = Vec::new();

			for filter in filters {
				match filter {
					FilterValue::Sort {
						index,
						ascending: asc,
						..
					} => {
						sort_index = index as usize;
						ascending = asc;
					}
					FilterValue::MultiSelect {
						id,
						included,
						excluded,
					} if id == "genre" => {
						included_genres = included;
						excluded_genres = excluded;
					}
					_ => {}
				}
			}

			helpers::browse_url(
				page,
				sort_index,
				ascending,
				&included_genres,
				&excluded_genres,
			)
		};

		Self::parse_manga_list(url)
	}

	fn get_manga_update(
		&self,
		mut manga: Manga,
		needs_details: bool,
		needs_chapters: bool,
	) -> Result<Manga> {
		let url = format!("{BASE_URL}{}", manga.key);
		let html = Request::get(&url)?.html()?;

		if needs_details {
			self.parse_details(&html, &mut manga, url);
			if needs_chapters {
				send_partial_result(&manga);
			}
		}

		if needs_chapters {
			manga.chapters = Some(self.fetch_chapters(&manga, &html)?);
		}

		Ok(manga)
	}

	fn get_page_list(&self, _manga: Manga, chapter: Chapter) -> Result<Vec<Page>> {
		let html = Request::get(helpers::reader_url(&chapter.key))?
			.header("Accept", "image/webp,image/apng")
			.string()?;
		let urls = helpers::page_urls(&html).ok_or(error!("Missing page list"))?;

		Ok(urls
			.into_iter()
			.map(|url| Page {
				content: PageContent::url(url),
				..Default::default()
			})
			.collect())
	}
}

impl ListingProvider for HenChan {
	fn get_manga_list(&self, listing: Listing, page: i32) -> Result<MangaPageResult> {
		let offset = 20 * (page - 1);
		let path = match listing.id.as_str() {
			"latest" => "manga/newest",
			"popular" => "mostfavorites",
			_ => return Ok(MangaPageResult::default()),
		};
		Self::parse_manga_list(format!("{BASE_URL}/{path}?offset={offset}"))
	}
}

impl DeepLinkHandler for HenChan {
	fn handle_deep_link(&self, url: String) -> Result<Option<DeepLinkResult>> {
		let path = url.split('?').next().unwrap_or(&url);
		if let Some(index) = path.find("/online/") {
			let chapter_key: String = path[index..].into();
			let manga_key = chapter_key.replacen("/online/", "/manga/", 1);
			return Ok(Some(DeepLinkResult::Chapter {
				manga_key,
				key: chapter_key,
			}));
		}

		Ok(helpers::manga_key(&url).map(|key| DeepLinkResult::Manga { key }))
	}
}

impl ImageRequestProvider for HenChan {
	fn get_image_request(&self, url: String, _context: Option<PageContext>) -> Result<Request> {
		Ok(Request::get(url)?
			.header("Accept", "image/webp,image/apng")
			.header("Referer", &format!("{BASE_URL}/")))
	}
}

register_source!(
	HenChan,
	ListingProvider,
	DeepLinkHandler,
	ImageRequestProvider
);
