use std::ops::Range;

use gpui::{
    div, img, px, AnyElement, App, ElementId, FontStyle, FontWeight, HighlightStyle,
    InteractiveText, IntoElement, ParentElement, SharedString, SharedUri, StrikethroughStyle,
    Styled, StyledImage, StyledText, TextStyle, UnderlineStyle, Window,
};
use gpui::prelude::FluentBuilder;
use gpui_component::theme::{ActiveTheme, Theme};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

pub fn render_markdown(
    markdown: &str,
    element_id_prefix: &str,
    window: &Window,
    cx: &App,
) -> AnyElement {
    let theme = cx.theme();
    let text_style = window.text_style();
    let mut renderer = MarkdownRenderer {
        theme,
        text_style,
        id_prefix: element_id_prefix.to_string(),
        id_counter: 0,
    };

    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let events: Vec<Event<'_>> = Parser::new_ext(markdown, options).collect();
    let blocks = renderer.render_blocks(&events);

    div()
        .flex()
        .flex_col()
        .gap(px(4.0))
        .w_full()
        .min_w_0()
        .children(blocks)
        .into_any_element()
}

struct MarkdownRenderer<'a> {
    theme: &'a Theme,
    text_style: TextStyle,
    id_prefix: String,
    id_counter: usize,
}

struct InlineContent {
    text: String,
    highlights: Vec<(Range<usize>, HighlightStyle)>,
    link_ranges: Vec<Range<usize>>,
    link_urls: Vec<String>,
}

impl<'a> MarkdownRenderer<'a> {
    fn next_id(&mut self) -> String {
        self.id_counter += 1;
        format!("{}-{}", self.id_prefix, self.id_counter)
    }

    fn render_blocks(&mut self, events: &[Event<'_>]) -> Vec<AnyElement> {
        let mut blocks = Vec::new();
        let mut cursor = 0;

        while cursor < events.len() {
            match &events[cursor] {
                Event::Start(Tag::Paragraph) => {
                    let (element, new_cursor) = self.render_paragraph(events, cursor);
                    blocks.push(element);
                    cursor = new_cursor;
                }
                Event::Start(Tag::Heading { level, .. }) => {
                    let (element, new_cursor) = self.render_heading(events, cursor, *level);
                    blocks.push(element);
                    cursor = new_cursor;
                }
                Event::Start(Tag::CodeBlock(_)) => {
                    let (element, new_cursor) = self.render_code_block(events, cursor);
                    blocks.push(element);
                    cursor = new_cursor;
                }
                Event::Start(Tag::List(start_num)) => {
                    let (element, new_cursor) = self.render_list(events, cursor, *start_num);
                    blocks.push(element);
                    cursor = new_cursor;
                }
                Event::Start(Tag::BlockQuote(_)) => {
                    let (element, new_cursor) = self.render_blockquote(events, cursor);
                    blocks.push(element);
                    cursor = new_cursor;
                }
                Event::Start(Tag::Table(_)) => {
                    let (element, new_cursor) = self.render_table(events, cursor);
                    blocks.push(element);
                    cursor = new_cursor;
                }
                Event::Rule => {
                    blocks.push(
                        div()
                            .w_full()
                            .h(px(1.0))
                            .bg(self.theme.border)
                            .my(px(8.0))
                            .into_any_element(),
                    );
                    cursor += 1;
                }
                _ => {
                    cursor += 1;
                }
            }
        }

        blocks
    }

    fn render_paragraph(&mut self, events: &[Event<'_>], start: usize) -> (AnyElement, usize) {
        let first = start + 1; // skip Start(Paragraph)

        // Check if paragraph contains only an image
        if let Some(Event::Start(Tag::Image { dest_url, .. })) = events.get(first) {
            let url = dest_url.to_string();
            let mut alt_text = String::new();
            let mut cursor = first + 1;
            while cursor < events.len() {
                match &events[cursor] {
                    Event::Text(t) => {
                        alt_text.push_str(t);
                        cursor += 1;
                    }
                    Event::End(TagEnd::Image) => {
                        cursor += 1;
                        break;
                    }
                    _ => {
                        cursor += 1;
                    }
                }
            }
            if let Some(Event::End(TagEnd::Paragraph)) = events.get(cursor) {
                cursor += 1;
                let element = self.render_block_image(&url, &alt_text);
                return (element, cursor);
            }
        }

        let (content, new_cursor) = self.collect_inline(events, first, TagEnd::Paragraph);
        let element = self.build_text_block(&content);
        (element, new_cursor)
    }

    fn render_block_image(&self, url: &str, alt_text: &str) -> AnyElement {
        log::info!("[markdown] rendering block image: url={}, alt={}", url, alt_text);
        let theme_muted_fg = self.theme.muted_foreground;
        let alt_owned = if alt_text.is_empty() {
            "[image]".to_string()
        } else {
            alt_text.to_string()
        };
        let url_for_log = url.to_string();
        div()
            .w_full()
            .child(
                img(SharedUri::from(url.to_string()))
                    .max_w_full()
                    .rounded(px(6.0))
                    .with_fallback(move || {
                        log::warn!("[markdown] image fallback triggered for: {}", url_for_log);
                        div()
                            .text_sm()
                            .text_color(theme_muted_fg)
                            .child(format!("[{}]", alt_owned))
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }

    fn render_heading(
        &mut self,
        events: &[Event<'_>],
        start: usize,
        level: HeadingLevel,
    ) -> (AnyElement, usize) {
        let end_tag = TagEnd::Heading(level);
        let (content, new_cursor) = self.collect_inline(events, start + 1, end_tag);

        let font_size = match level {
            HeadingLevel::H1 => px(24.0),
            HeadingLevel::H2 => px(20.0),
            HeadingLevel::H3 => px(18.0),
            HeadingLevel::H4 => px(16.0),
            HeadingLevel::H5 => px(14.0),
            HeadingLevel::H6 => px(14.0),
        };

        // Add bold to all heading highlights
        let bold_hl = HighlightStyle {
            font_weight: Some(FontWeight::BOLD),
            ..Default::default()
        };
        let highlights: Vec<_> = content
            .highlights
            .iter()
            .map(|(r, hl)| (r.clone(), merge_highlight_styles(&[*hl, bold_hl])))
            .collect();
        // Fill gaps with bold-only
        let highlights = fill_gaps_with_default(&content.text, &highlights, bold_hl);

        let id = self.next_id();
        let text_element = build_interactive_or_styled(
            &id,
            &content.text,
            &highlights,
            &content.link_ranges,
            &content.link_urls,
            &self.text_style,
        );

        let has_border = matches!(level, HeadingLevel::H1 | HeadingLevel::H2);
        let text_color = if level == HeadingLevel::H6 {
            self.theme.muted_foreground
        } else {
            self.theme.foreground
        };

        let element = div()
            .w_full()
            .min_w_0()
            .text_size(font_size)
            .text_color(text_color)
            .whitespace_normal()
            .pt(px(8.0))
            .pb(px(4.0))
            .when(has_border, |el| {
                el.border_b_1()
                    .border_color(self.theme.border)
                    .pb(px(8.0))
            })
            .child(text_element)
            .into_any_element();

        (element, new_cursor)
    }

    fn render_code_block(&mut self, events: &[Event<'_>], start: usize) -> (AnyElement, usize) {
        let mut cursor = start + 1;
        let mut code = String::new();

        while cursor < events.len() {
            match &events[cursor] {
                Event::Text(t) => {
                    code.push_str(t);
                    cursor += 1;
                }
                Event::End(TagEnd::CodeBlock) => {
                    cursor += 1;
                    break;
                }
                _ => {
                    cursor += 1;
                }
            }
        }

        if code.ends_with('\n') {
            code.pop();
        }

        let element = div()
            .w_full()
            .min_w_0()
            .bg(self.theme.muted.opacity(0.2))
            .rounded(px(6.0))
            .p(px(12.0))
            .font_family("Berkeley Mono, monospace")
            .text_sm()
            .text_color(self.theme.foreground)
            .whitespace_normal()
            .overflow_hidden()
            .child(code)
            .into_any_element();

        (element, cursor)
    }

    fn render_list(
        &mut self,
        events: &[Event<'_>],
        start: usize,
        start_num: Option<u64>,
    ) -> (AnyElement, usize) {
        let mut cursor = start + 1;
        let mut items = Vec::new();
        let mut item_index: u64 = start_num.unwrap_or(1);
        let is_ordered = start_num.is_some();

        while cursor < events.len() {
            match &events[cursor] {
                Event::Start(Tag::Item) => {
                    let (item_element, new_cursor) =
                        self.render_list_item(events, cursor, is_ordered, item_index);
                    items.push(item_element);
                    cursor = new_cursor;
                    item_index += 1;
                }
                Event::End(TagEnd::List(_)) => {
                    cursor += 1;
                    break;
                }
                _ => {
                    cursor += 1;
                }
            }
        }

        let element = div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .gap(px(2.0))
            .children(items)
            .into_any_element();

        (element, cursor)
    }

    fn render_list_item(
        &mut self,
        events: &[Event<'_>],
        start: usize,
        is_ordered: bool,
        index: u64,
    ) -> (AnyElement, usize) {
        let mut cursor = start + 1;
        let mut children = Vec::new();

        let marker = if is_ordered {
            format!("{}.", index)
        } else {
            "\u{2022}".to_string()
        };

        while cursor < events.len() {
            match &events[cursor] {
                Event::Start(Tag::Paragraph) => {
                    let (content, new_cursor) =
                        self.collect_inline(events, cursor + 1, TagEnd::Paragraph);
                    children.push(self.build_text_block(&content));
                    cursor = new_cursor;
                }
                Event::Start(Tag::List(sub_start)) => {
                    let (sub_list, new_cursor) = self.render_list(events, cursor, *sub_start);
                    children.push(sub_list);
                    cursor = new_cursor;
                }
                Event::End(TagEnd::Item) => {
                    cursor += 1;
                    break;
                }
                Event::Text(_)
                | Event::Code(_)
                | Event::Start(Tag::Emphasis)
                | Event::Start(Tag::Strong)
                | Event::Start(Tag::Strikethrough)
                | Event::Start(Tag::Link { .. }) => {
                    let (content, new_cursor) =
                        self.collect_inline(events, cursor, TagEnd::Item);
                    children.push(self.build_text_block(&content));
                    cursor = new_cursor;
                    break;
                }
                _ => {
                    cursor += 1;
                }
            }
        }

        let element = div()
            .w_full()
            .min_w_0()
            .flex()
            .gap(px(6.0))
            .child(
                div()
                    .text_sm()
                    .text_color(self.theme.muted_foreground)
                    .flex_shrink_0()
                    .w(px(if is_ordered { 20.0 } else { 12.0 }))
                    .text_right()
                    .child(marker),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .gap(px(2.0))
                    .children(children),
            )
            .into_any_element();

        (element, cursor)
    }

    fn render_blockquote(&mut self, events: &[Event<'_>], start: usize) -> (AnyElement, usize) {
        let mut cursor = start + 1;
        let mut depth = 1;
        let mut inner_events = Vec::new();

        while cursor < events.len() && depth > 0 {
            match &events[cursor] {
                Event::Start(Tag::BlockQuote(_)) => {
                    depth += 1;
                    inner_events.push(events[cursor].clone());
                }
                Event::End(TagEnd::BlockQuote(_)) => {
                    depth -= 1;
                    if depth > 0 {
                        inner_events.push(events[cursor].clone());
                    }
                }
                _ => {
                    inner_events.push(events[cursor].clone());
                }
            }
            cursor += 1;
        }

        let inner_blocks = self.render_blocks(&inner_events);

        let element = div()
            .w_full()
            .min_w_0()
            .border_l(px(3.0))
            .border_color(self.theme.border)
            .pl(px(12.0))
            .text_color(self.theme.muted_foreground)
            .flex()
            .flex_col()
            .gap(px(4.0))
            .children(inner_blocks)
            .into_any_element();

        (element, cursor)
    }

    fn render_table(&mut self, events: &[Event<'_>], start: usize) -> (AnyElement, usize) {
        let mut cursor = start + 1;
        let mut header_cells: Vec<InlineContent> = Vec::new();
        let mut body_rows: Vec<Vec<InlineContent>> = Vec::new();
        let mut in_head = false;
        let mut current_row: Vec<InlineContent> = Vec::new();

        while cursor < events.len() {
            match &events[cursor] {
                Event::Start(Tag::TableHead) => {
                    in_head = true;
                    cursor += 1;
                }
                Event::End(TagEnd::TableHead) => {
                    in_head = false;
                    header_cells = current_row;
                    current_row = Vec::new();
                    cursor += 1;
                }
                Event::Start(Tag::TableRow) => {
                    current_row = Vec::new();
                    cursor += 1;
                }
                Event::End(TagEnd::TableRow) => {
                    if !in_head {
                        body_rows.push(current_row);
                        current_row = Vec::new();
                    }
                    cursor += 1;
                }
                Event::Start(Tag::TableCell) => {
                    let (content, new_cursor) =
                        self.collect_inline(events, cursor + 1, TagEnd::TableCell);
                    current_row.push(content);
                    cursor = new_cursor;
                }
                Event::End(TagEnd::Table) => {
                    cursor += 1;
                    break;
                }
                _ => {
                    cursor += 1;
                }
            }
        }

        let col_count = header_cells.len().max(1);

        let header_element = div()
            .flex()
            .border_b_1()
            .border_color(self.theme.border)
            .bg(self.theme.muted.opacity(0.15))
            .children(header_cells.iter().map(|cell| {
                let bold_hl = HighlightStyle {
                    font_weight: Some(FontWeight::BOLD),
                    ..Default::default()
                };
                let highlights: Vec<_> = cell
                    .highlights
                    .iter()
                    .map(|(r, hl)| (r.clone(), merge_highlight_styles(&[*hl, bold_hl])))
                    .collect();
                let highlights = fill_gaps_with_default(&cell.text, &highlights, bold_hl);

                let id = self.next_id();
                let text_el = build_interactive_or_styled(
                    &id,
                    &cell.text,
                    &highlights,
                    &cell.link_ranges,
                    &cell.link_urls,
                    &self.text_style,
                );
                div()
                    .flex_1()
                    .min_w(px(80.0))
                    .px(px(12.0))
                    .py(px(8.0))
                    .text_sm()
                    .child(text_el)
            }))
            .into_any_element();

        let body_elements: Vec<AnyElement> = body_rows
            .iter()
            .map(|row| {
                div()
                    .flex()
                    .border_b_1()
                    .border_color(self.theme.border)
                    .children((0..col_count).map(|i| {
                        if let Some(cell) = row.get(i) {
                            let id = self.next_id();
                            let text_el = build_interactive_or_styled(
                                &id,
                                &cell.text,
                                &cell.highlights,
                                &cell.link_ranges,
                                &cell.link_urls,
                                &self.text_style,
                            );
                            div()
                                .flex_1()
                                .min_w(px(80.0))
                                .px(px(12.0))
                                .py(px(8.0))
                                .text_sm()
                                .child(text_el)
                                .into_any_element()
                        } else {
                            div()
                                .flex_1()
                                .min_w(px(80.0))
                                .px(px(12.0))
                                .py(px(8.0))
                                .into_any_element()
                        }
                    }))
                    .into_any_element()
            })
            .collect();

        let element = div()
            .w_full()
            .min_w_0()
            .flex()
            .flex_col()
            .rounded(px(6.0))
            .border_1()
            .border_color(self.theme.border)
            .overflow_hidden()
            .child(header_element)
            .children(body_elements)
            .into_any_element();

        (element, cursor)
    }

    /// Collect inline events into an InlineContent with properly merged,
    /// non-overlapping highlight ranges.
    fn collect_inline(
        &mut self,
        events: &[Event<'_>],
        start: usize,
        end_tag: TagEnd,
    ) -> (InlineContent, usize) {
        let mut text = String::new();
        let mut link_ranges: Vec<Range<usize>> = Vec::new();
        let mut link_urls: Vec<String> = Vec::new();
        let mut cursor = start;

        // Track style spans as (byte_start, byte_end, style) entries.
        // Each text segment gets ONE merged highlight covering all active styles.
        let mut highlights: Vec<(Range<usize>, HighlightStyle)> = Vec::new();

        let mut bold_depth = 0u32;
        let mut italic_depth = 0u32;
        let mut strikethrough_depth = 0u32;
        let mut current_link: Option<String> = None;
        let mut link_start: usize = 0;

        while cursor < events.len() {
            match &events[cursor] {
                Event::End(tag) if *tag == end_tag => {
                    cursor += 1;
                    break;
                }
                Event::Text(t) => {
                    let start_offset = text.len();
                    text.push_str(t);
                    let end_offset = text.len();

                    let hl = self.current_highlight(
                        bold_depth,
                        italic_depth,
                        strikethrough_depth,
                        current_link.is_some(),
                    );
                    if let Some(hl) = hl {
                        highlights.push((start_offset..end_offset, hl));
                    }
                    cursor += 1;
                }
                Event::Code(code) => {
                    let start_offset = text.len();
                    text.push_str(code);
                    let end_offset = text.len();

                    let code_hl = HighlightStyle {
                        background_color: Some(self.theme.muted.opacity(0.3)),
                        ..Default::default()
                    };
                    let base = self.current_highlight(
                        bold_depth,
                        italic_depth,
                        strikethrough_depth,
                        current_link.is_some(),
                    );
                    let merged = if let Some(base) = base {
                        merge_highlight_styles(&[base, code_hl])
                    } else {
                        code_hl
                    };
                    highlights.push((start_offset..end_offset, merged));
                    cursor += 1;
                }
                Event::SoftBreak => {
                    text.push(' ');
                    cursor += 1;
                }
                Event::HardBreak => {
                    text.push('\n');
                    cursor += 1;
                }
                Event::Start(Tag::Strong) => {
                    bold_depth += 1;
                    cursor += 1;
                }
                Event::End(TagEnd::Strong) => {
                    bold_depth = bold_depth.saturating_sub(1);
                    cursor += 1;
                }
                Event::Start(Tag::Emphasis) => {
                    italic_depth += 1;
                    cursor += 1;
                }
                Event::End(TagEnd::Emphasis) => {
                    italic_depth = italic_depth.saturating_sub(1);
                    cursor += 1;
                }
                Event::Start(Tag::Strikethrough) => {
                    strikethrough_depth += 1;
                    cursor += 1;
                }
                Event::End(TagEnd::Strikethrough) => {
                    strikethrough_depth = strikethrough_depth.saturating_sub(1);
                    cursor += 1;
                }
                Event::Start(Tag::Link { dest_url, .. }) => {
                    current_link = Some(dest_url.to_string());
                    link_start = text.len();
                    cursor += 1;
                }
                Event::End(TagEnd::Link) => {
                    if let Some(url) = current_link.take() {
                        let range = link_start..text.len();
                        if !range.is_empty() {
                            link_ranges.push(range);
                            link_urls.push(url);
                        }
                    }
                    cursor += 1;
                }
                Event::Start(Tag::Image { dest_url, .. }) => {
                    let url = dest_url.to_string();
                    let mut alt = String::new();
                    cursor += 1;
                    while cursor < events.len() {
                        match &events[cursor] {
                            Event::Text(t) => {
                                alt.push_str(t);
                                cursor += 1;
                            }
                            Event::End(TagEnd::Image) => {
                                cursor += 1;
                                break;
                            }
                            _ => {
                                cursor += 1;
                            }
                        }
                    }
                    let label = if alt.is_empty() {
                        "[image]".to_string()
                    } else {
                        alt
                    };
                    let start_offset = text.len();
                    text.push_str(&label);
                    let end_offset = text.len();
                    highlights.push((
                        start_offset..end_offset,
                        HighlightStyle {
                            color: Some(self.theme.link),
                            underline: Some(UnderlineStyle {
                                thickness: px(1.0),
                                color: Some(self.theme.link),
                                wavy: false,
                            }),
                            ..Default::default()
                        },
                    ));
                    link_ranges.push(start_offset..end_offset);
                    link_urls.push(url);
                }
                _ => {
                    cursor += 1;
                }
            }
        }

        (
            InlineContent {
                text,
                highlights,
                link_ranges,
                link_urls,
            },
            cursor,
        )
    }

    /// Build a single merged HighlightStyle from the currently active inline styles.
    /// Returns None if no styles are active.
    fn current_highlight(
        &self,
        bold_depth: u32,
        italic_depth: u32,
        strikethrough_depth: u32,
        in_link: bool,
    ) -> Option<HighlightStyle> {
        let mut hl = HighlightStyle::default();
        let mut any = false;

        if bold_depth > 0 {
            hl.font_weight = Some(FontWeight::BOLD);
            any = true;
        }
        if italic_depth > 0 {
            hl.font_style = Some(FontStyle::Italic);
            any = true;
        }
        if strikethrough_depth > 0 {
            hl.strikethrough = Some(StrikethroughStyle {
                thickness: px(1.0),
                color: None,
            });
            any = true;
        }
        if in_link {
            hl.color = Some(self.theme.link);
            hl.underline = Some(UnderlineStyle {
                thickness: px(1.0),
                color: Some(self.theme.link),
                wavy: false,
            });
            any = true;
        }

        if any { Some(hl) } else { None }
    }

    fn build_text_block(&mut self, content: &InlineContent) -> AnyElement {
        if content.text.is_empty() {
            return div().into_any_element();
        }

        let id = self.next_id();
        let text_element = build_interactive_or_styled(
            &id,
            &content.text,
            &content.highlights,
            &content.link_ranges,
            &content.link_urls,
            &self.text_style,
        );

        div()
            .w_full()
            .min_w_0()
            .text_sm()
            .text_color(self.theme.foreground)
            .whitespace_normal()
            .child(text_element)
            .into_any_element()
    }
}

/// Merge multiple HighlightStyles into one by combining their fields.
fn merge_highlight_styles(styles: &[HighlightStyle]) -> HighlightStyle {
    let mut result = HighlightStyle::default();
    for s in styles {
        if let Some(w) = s.font_weight {
            result.font_weight = Some(w);
        }
        if let Some(fs) = s.font_style {
            result.font_style = Some(fs);
        }
        if let Some(c) = s.color {
            result.color = Some(c);
        }
        if let Some(bg) = s.background_color {
            result.background_color = Some(bg);
        }
        if let Some(u) = s.underline {
            result.underline = Some(u);
        }
        if let Some(st) = s.strikethrough {
            result.strikethrough = Some(st);
        }
        if let Some(fo) = s.fade_out {
            result.fade_out = Some(fo);
        }
    }
    result
}

/// Fill gaps between highlight ranges with a default highlight, producing
/// a complete non-overlapping coverage of the entire text.
fn fill_gaps_with_default(
    text: &str,
    highlights: &[(Range<usize>, HighlightStyle)],
    default_hl: HighlightStyle,
) -> Vec<(Range<usize>, HighlightStyle)> {
    let mut result = Vec::new();
    let mut ix = 0;
    for (range, hl) in highlights {
        if ix < range.start {
            result.push((ix..range.start, default_hl));
        }
        result.push((range.clone(), *hl));
        ix = range.end;
    }
    if ix < text.len() {
        result.push((ix..text.len(), default_hl));
    }
    result
}

fn build_interactive_or_styled(
    id: &str,
    text: &str,
    highlights: &[(Range<usize>, HighlightStyle)],
    link_ranges: &[Range<usize>],
    link_urls: &[String],
    text_style: &TextStyle,
) -> AnyElement {
    let styled = StyledText::new(SharedString::from(text.to_string()))
        .with_default_highlights(text_style, highlights.iter().cloned());

    if link_ranges.is_empty() {
        styled.into_any_element()
    } else {
        let urls: Vec<String> = link_urls.to_vec();
        InteractiveText::new(ElementId::from(SharedString::from(id.to_string())), styled)
            .on_click(link_ranges.to_vec(), move |ix, _window, cx| {
                if let Some(url) = urls.get(ix) {
                    cx.open_url(url);
                }
            })
            .into_any_element()
    }
}
