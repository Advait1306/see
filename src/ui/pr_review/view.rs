use gpui::prelude::*;
use gpui::*;
use gpui_component::input::{Input, InputState};
use gpui_component::theme::ActiveTheme;
use serde::{Deserialize, Serialize};

use crate::commands::{PrReviewNextFile, PrReviewPrevFile};
use crate::github::{CreateReviewComment, CreateReviewRequest, PullRequestFile, ReviewComment};
use crate::stores::{Buffer, DiffLineTag, GitHubStore, GitHubStoreEvent};
use crate::types::{Tab, TabConfig};
use crate::ui::editor::{DiffDisplayLine, EditorView, EditorViewEvent};

use super::file_diff;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PrReviewTabConfig {
    pub owner: String,
    pub repo: String,
    pub pr_number: u64,
    pub pr_title: String,
}

#[derive(Debug, Clone, PartialEq)]
struct CommentAnchor {
    path: String,
    line: u64,
    side: String,
    display_line_index: usize,
}

struct PendingComment {
    anchor: CommentAnchor,
    body: String,
}

pub struct PrReviewView {
    github_store: Entity<GitHubStore>,
    pr_number: u64,
    pr_title: String,
    owner: String,
    repo: String,
    selected_file_index: usize,
    diff_editor: Option<Entity<EditorView>>,
    focus_handle: FocusHandle,
    _subscription: Subscription,
    // Comment support
    pending_comments: Vec<PendingComment>,
    active_comment_anchor: Option<CommentAnchor>,
    active_comment_existing: Vec<ReviewComment>,
    active_comment_pending_body: Option<String>,
    comment_input_state: Option<Entity<InputState>>,
    submitting: bool,
    _editor_subscription: Option<Subscription>,
}

impl PrReviewView {
    pub fn new(
        github_store: Entity<GitHubStore>,
        pr_number: u64,
        pr_title: String,
        cx: &mut Context<Self>,
    ) -> Self {
        let owner = github_store.read(cx).owner().to_string();
        let repo = github_store.read(cx).repo().to_string();

        let sub = cx.subscribe(&github_store, |this, _store, event, cx| {
            match event {
                GitHubStoreEvent::PrDetailsUpdated(num) if *num == this.pr_number => {
                    if this.diff_editor.is_none() {
                        this.rebuild_diff_editor(cx);
                    }
                    this.update_comment_markers(cx);
                    cx.notify();
                }
                _ => {
                    cx.notify();
                }
            }
        });

        github_store.update(cx, |store, cx| {
            store.load_pr_details(pr_number, cx);
        });

        Self {
            github_store,
            pr_number,
            pr_title,
            owner,
            repo,
            selected_file_index: 0,
            diff_editor: None,
            focus_handle: cx.focus_handle(),
            _subscription: sub,
            pending_comments: Vec::new(),
            active_comment_anchor: None,
            active_comment_existing: Vec::new(),
            active_comment_pending_body: None,
            comment_input_state: None,
            submitting: false,
            _editor_subscription: None,
        }
    }

    fn files(&self, cx: &App) -> Vec<PullRequestFile> {
        let store = self.github_store.read(cx);
        if let Some(details) = store.pr_details(self.pr_number) {
            details.files.clone()
        } else {
            Vec::new()
        }
    }

    fn selected_filename(&self, cx: &App) -> Option<String> {
        let files = self.files(cx);
        files.get(self.selected_file_index).map(|f| f.filename.clone())
    }

    fn select_file(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_file_index = index;
        self.close_comment_panel(cx);
        self.rebuild_diff_editor(cx);
        cx.notify();
    }

    fn rebuild_diff_editor(&mut self, cx: &mut Context<Self>) {
        let files = self.files(cx);
        let file = match files.get(self.selected_file_index) {
            Some(f) => f,
            None => {
                self.diff_editor = None;
                self._editor_subscription = None;
                return;
            }
        };

        let patch = match &file.patch {
            Some(p) => p.clone(),
            None => {
                self.diff_editor = None;
                self._editor_subscription = None;
                return;
            }
        };

        let filename = file.filename.clone();
        let (content, diff_lines, buffer_line_map) = file_diff::patch_to_editor_diff_lines(&patch);

        let buffer = cx.new(|cx| Buffer::from_content(content, &filename, cx));
        let editor = cx.new(|cx| {
            EditorView::new_external_diff(buffer, diff_lines, buffer_line_map, cx)
        });

        let sub = cx.subscribe(&editor, |this, _editor, event, cx| {
            let EditorViewEvent::DiffLineClicked {
                display_line_index,
                old_line_num,
                new_line_num,
                tag,
            } = event;
            this.on_diff_line_clicked(
                *display_line_index,
                *old_line_num,
                *new_line_num,
                *tag,
                cx,
            );
        });

        self.diff_editor = Some(editor);
        self._editor_subscription = Some(sub);
        self.update_comment_markers(cx);
    }

    pub fn next_file(&mut self, cx: &mut Context<Self>) {
        let count = self.files(cx).len();
        if count > 0 {
            self.selected_file_index = (self.selected_file_index + 1) % count;
            self.close_comment_panel(cx);
            self.rebuild_diff_editor(cx);
            cx.notify();
        }
    }

    pub fn prev_file(&mut self, cx: &mut Context<Self>) {
        let count = self.files(cx).len();
        if count > 0 {
            self.selected_file_index = if self.selected_file_index == 0 {
                count - 1
            } else {
                self.selected_file_index - 1
            };
            self.close_comment_panel(cx);
            self.rebuild_diff_editor(cx);
            cx.notify();
        }
    }

    fn on_diff_line_clicked(
        &mut self,
        display_line_index: usize,
        old_line_num: Option<usize>,
        new_line_num: Option<usize>,
        tag: DiffLineTag,
        cx: &mut Context<Self>,
    ) {
        let path = match self.selected_filename(cx) {
            Some(p) => p,
            None => return,
        };

        let (line, side) = match tag {
            DiffLineTag::Delete => {
                if let Some(old) = old_line_num {
                    (old as u64, "LEFT".to_string())
                } else {
                    return;
                }
            }
            DiffLineTag::Insert | DiffLineTag::Equal => {
                if let Some(new) = new_line_num {
                    (new as u64, "RIGHT".to_string())
                } else {
                    return;
                }
            }
        };

        let anchor = CommentAnchor {
            path: path.clone(),
            line,
            side: side.clone(),
            display_line_index,
        };

        // Look up existing comments for this line
        let existing = self.existing_comments_for(&path, line, cx);

        // Look up pending comment body for this anchor
        let pending_body = self
            .pending_comments
            .iter()
            .find(|pc| pc.anchor == anchor)
            .map(|pc| pc.body.clone());

        // Reset input state with pending body (if any)
        // Input will be created lazily in render if needed
        self.comment_input_state = None;

        self.active_comment_anchor = Some(anchor);
        self.active_comment_existing = existing;

        // Store pending body to populate input when created
        if let Some(body) = pending_body {
            self.active_comment_pending_body = Some(body);
        } else {
            self.active_comment_pending_body = None;
        }

        cx.notify();
    }

    fn existing_comments_for(&self, path: &str, line: u64, cx: &App) -> Vec<ReviewComment> {
        let store = self.github_store.read(cx);
        if let Some(details) = store.pr_details(self.pr_number) {
            details
                .comments
                .iter()
                .filter(|c| c.path == path && c.line == Some(line))
                .cloned()
                .collect()
        } else {
            Vec::new()
        }
    }

    fn save_pending_comment(&mut self, cx: &mut Context<Self>) {
        let anchor = match &self.active_comment_anchor {
            Some(a) => a.clone(),
            None => return,
        };

        let body = if let Some(ref input) = self.comment_input_state {
            input.read(cx).value().to_string()
        } else {
            return;
        };

        if body.trim().is_empty() {
            return;
        }

        // Upsert
        if let Some(existing) = self
            .pending_comments
            .iter_mut()
            .find(|pc| pc.anchor == anchor)
        {
            existing.body = body;
        } else {
            self.pending_comments.push(PendingComment {
                anchor,
                body,
            });
        }

        self.close_comment_panel(cx);
        self.update_comment_markers(cx);
        cx.notify();
    }

    fn close_comment_panel(&mut self, _cx: &mut Context<Self>) {
        self.active_comment_anchor = None;
        self.active_comment_existing.clear();
        self.active_comment_pending_body = None;
        self.comment_input_state = None;
    }

    fn update_comment_markers(&mut self, cx: &mut Context<Self>) {
        let editor = match &self.diff_editor {
            Some(e) => e.clone(),
            None => return,
        };

        let path = match self.selected_filename(cx) {
            Some(p) => p,
            None => return,
        };

        let mut marker_indices: Vec<usize> = Vec::new();

        // Add indices from pending comments for the current file
        for pc in &self.pending_comments {
            if pc.anchor.path == path {
                marker_indices.push(pc.anchor.display_line_index);
            }
        }

        // Add indices from existing GitHub comments
        let store = self.github_store.read(cx);
        if let Some(details) = store.pr_details(self.pr_number) {
            // Map existing comments to display line indices
            for comment in &details.comments {
                if comment.path != path {
                    continue;
                }
                if let Some(line) = comment.line {
                    // Find the display line index that matches this line number
                    let side = comment.side.as_deref().unwrap_or("RIGHT");
                    if let Some(idx) = self.find_display_line_index(line, side, cx) {
                        marker_indices.push(idx);
                    }
                }
            }
        }

        marker_indices.sort();
        marker_indices.dedup();

        editor.update(cx, |view, _cx| {
            view.set_comment_lines(marker_indices);
        });
    }

    fn find_display_line_index(&self, line: u64, side: &str, cx: &App) -> Option<usize> {
        let editor = self.diff_editor.as_ref()?;
        let editor = editor.read(cx);
        let diff_data = editor.diff_mode.as_ref()?;

        for (idx, display_line) in diff_data.display_lines.iter().enumerate() {
            if let DiffDisplayLine::Line { line: diff_line, .. } = display_line {
                let matches = if side == "LEFT" {
                    diff_line.old_line_num == Some(line as usize)
                        && diff_line.tag == DiffLineTag::Delete
                } else {
                    diff_line.new_line_num == Some(line as usize)
                        && diff_line.tag != DiffLineTag::Delete
                };
                if matches {
                    return Some(idx);
                }
            }
        }
        None
    }

    fn submit_review(&mut self, event: &str, cx: &mut Context<Self>) {
        if self.submitting {
            return;
        }
        self.submitting = true;

        let comments: Vec<CreateReviewComment> = self
            .pending_comments
            .iter()
            .map(|pc| CreateReviewComment {
                path: pc.anchor.path.clone(),
                body: pc.body.clone(),
                line: pc.anchor.line,
                side: pc.anchor.side.clone(),
            })
            .collect();

        let request = CreateReviewRequest {
            body: String::new(),
            event: event.to_string(),
            comments,
        };

        let pr_number = self.pr_number;
        self.github_store.update(cx, |store, cx| {
            store.submit_review(pr_number, request, cx);
        });

        self.pending_comments.clear();
        self.close_comment_panel(cx);
        self.submitting = false;
        cx.notify();
    }

    fn render_header(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        let pending_count = self.pending_comments.len();

        div()
            .debug_selector(|| "pr-review-header".into())
            .w_full()
            .h(px(40.0))
            .flex()
            .items_center()
            .px_3()
            .gap_2()
            .border_b_1()
            .border_color(theme.border)
            .bg(theme.tab_bar)
            .child(
                div()
                    .text_sm()
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(theme.foreground)
                    .flex_1()
                    .overflow_hidden()
                    .child(format!("#{} {}", self.pr_number, self.pr_title)),
            )
            // Pending count badge
            .when(pending_count > 0, |el| {
                el.child(
                    div()
                        .text_xs()
                        .px_2()
                        .py(px(2.0))
                        .rounded_md()
                        .bg(theme.info)
                        .text_color(theme.primary_foreground)
                        .child(format!("{} pending", pending_count)),
                )
            })
            // Review action buttons
            .child(
                div()
                    .id("review-comment-btn")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .border_1()
                    .border_color(theme.border)
                    .text_color(theme.foreground)
                    .hover(|s| s.bg(theme.secondary))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.submit_review("COMMENT", cx);
                    }))
                    .child("Comment"),
            )
            .child(
                div()
                    .id("review-approve-btn")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .bg(theme.success)
                    .text_color(theme.primary_foreground)
                    .hover(|s| s.opacity(0.9))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.submit_review("APPROVE", cx);
                    }))
                    .child("Approve"),
            )
            .child(
                div()
                    .id("review-request-changes-btn")
                    .cursor_pointer()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .text_xs()
                    .border_1()
                    .border_color(theme.danger)
                    .text_color(theme.danger)
                    .hover(|s| s.bg(theme.danger.opacity(0.1)))
                    .on_click(cx.listener(|this, _, _window, cx| {
                        this.submit_review("REQUEST_CHANGES", cx);
                    }))
                    .child("Request Changes"),
            )
            .into_any_element()
    }

    fn render_file_item(
        &self,
        idx: usize,
        file: &PullRequestFile,
        cx: &Context<Self>,
    ) -> AnyElement {
        let is_selected = idx == self.selected_file_index;
        let theme = cx.theme();

        let filename = file
            .filename
            .rsplit('/')
            .next()
            .unwrap_or(&file.filename)
            .to_string();

        let additions = file.additions;
        let deletions = file.deletions;

        // Check if this file has pending comments
        let has_pending = self
            .pending_comments
            .iter()
            .any(|pc| pc.anchor.path == file.filename);

        div()
            .id(ElementId::Name(format!("pr-file-{}", idx).into()))
            .w_full()
            .px_2()
            .py_1()
            .cursor_pointer()
            .when(is_selected, |el| el.bg(theme.secondary))
            .when(!is_selected, |el| el.hover(|s| s.bg(theme.secondary)))
            .on_click(cx.listener(move |this, _, _window, cx| {
                this.select_file(idx, cx);
            }))
            .flex()
            .items_center()
            .gap_2()
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .text_xs()
                    .text_color(if is_selected {
                        theme.foreground
                    } else {
                        theme.muted_foreground
                    })
                    .child(filename),
            )
            .when(has_pending, |el| {
                el.child(
                    div()
                        .w(px(6.0))
                        .h(px(6.0))
                        .rounded_full()
                        .bg(theme.info),
                )
            })
            .child(
                div()
                    .flex()
                    .gap_1()
                    .text_xs()
                    .child(
                        div()
                            .text_color(theme.success)
                            .child(format!("+{}", additions)),
                    )
                    .child(
                        div()
                            .text_color(theme.danger)
                            .child(format!("-{}", deletions)),
                    ),
            )
            .into_any_element()
    }

    fn render_file_list(&self, files: &[PullRequestFile], cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();

        let file_items: Vec<AnyElement> = files
            .iter()
            .enumerate()
            .map(|(idx, file)| self.render_file_item(idx, file, cx))
            .collect();

        div()
            .debug_selector(|| "pr-file-list".into())
            .w(px(220.0))
            .h_full()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .border_r_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .child(
                div()
                    .h(px(28.0))
                    .w_full()
                    .flex()
                    .items_center()
                    .px_2()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .child(format!("FILES ({})", files.len())),
                    ),
            )
            .children(file_items)
            .into_any_element()
    }

    fn render_comment_panel(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let anchor = match &self.active_comment_anchor {
            Some(a) => a.clone(),
            None => return div().into_any_element(),
        };

        // Create input state lazily (must happen before borrowing theme)
        if self.comment_input_state.is_none() {
            let initial_text = self.active_comment_pending_body.clone().unwrap_or_default();
            let input = cx.new(|cx| {
                let mut state = InputState::new(window, cx)
                    .multi_line(true)
                    .placeholder("Write a comment...");
                if !initial_text.is_empty() {
                    state.set_value(&initial_text, window, cx);
                }
                state
            });
            self.comment_input_state = Some(input);
        }

        let theme = cx.theme();
        let input_state = self.comment_input_state.as_ref().unwrap().clone();
        let line_label = format!("Line {} in {}", anchor.line, anchor.path);

        // Existing comments
        let existing_elements: Vec<AnyElement> = self
            .active_comment_existing
            .iter()
            .map(|c| {
                let timestamp = format_timestamp(&c.created_at);
                let header = format!("@{} ({})", c.user.login, timestamp);
                div()
                    .w_full()
                    .px_3()
                    .py_1()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(theme.muted_foreground)
                            .child(header),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.foreground)
                            .child(c.body.clone()),
                    )
                    .into_any_element()
            })
            .collect();

        let has_existing = !existing_elements.is_empty();

        div()
            .w_full()
            .border_t_1()
            .border_color(theme.border)
            .bg(theme.sidebar)
            .flex()
            .flex_col()
            // Header
            .child(
                div()
                    .w_full()
                    .h(px(28.0))
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .border_b_1()
                    .border_color(theme.border)
                    .child(
                        div()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .child(line_label),
                    )
                    .child(
                        div()
                            .id("comment-panel-close")
                            .cursor_pointer()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .hover(|s| s.text_color(theme.foreground))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.close_comment_panel(cx);
                                cx.notify();
                            }))
                            .child("X"),
                    ),
            )
            // Existing comments
            .when(has_existing, |el| {
                el.child(
                    div()
                        .w_full()
                        .max_h(px(120.0))
                        .overflow_hidden()
                        .border_b_1()
                        .border_color(theme.border)
                        .children(existing_elements),
                )
            })
            // Input area
            .child(
                div()
                    .w_full()
                    .p_2()
                    .child(
                        Input::new(&input_state)
                            .appearance(true)
                            .h(px(60.0)),
                    ),
            )
            // Buttons
            .child(
                div()
                    .w_full()
                    .flex()
                    .justify_end()
                    .gap_2()
                    .px_3()
                    .pb_2()
                    .child(
                        div()
                            .id("comment-cancel-btn")
                            .cursor_pointer()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .text_color(theme.muted_foreground)
                            .border_1()
                            .border_color(theme.border)
                            .hover(|s| s.bg(theme.secondary))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.close_comment_panel(cx);
                                cx.notify();
                            }))
                            .child("Cancel"),
                    )
                    .child(
                        div()
                            .id("comment-save-btn")
                            .cursor_pointer()
                            .px_3()
                            .py_1()
                            .rounded_md()
                            .text_xs()
                            .text_color(theme.primary_foreground)
                            .bg(theme.primary)
                            .hover(|s| s.opacity(0.9))
                            .on_click(cx.listener(|this, _, _window, cx| {
                                this.save_pending_comment(cx);
                            }))
                            .child("Add Comment"),
                    ),
            )
            .into_any_element()
    }

    fn render_diff_area(
        &mut self,
        files: &[PullRequestFile],
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        if let Some(file) = files.get(self.selected_file_index) {
            if let Some(editor) = &self.diff_editor {
                let has_comment_panel = self.active_comment_anchor.is_some();
                let editor = editor.clone();
                let filename = file.filename.clone();

                let comment_panel = if has_comment_panel {
                    self.render_comment_panel(window, cx)
                } else {
                    div().into_any_element()
                };

                let theme = cx.theme();
                return div()
                    .debug_selector(|| "pr-diff-area".into())
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .overflow_hidden()
                    .child(
                        div()
                            .h(px(28.0))
                            .w_full()
                            .flex()
                            .items_center()
                            .px_3()
                            .border_b_1()
                            .border_color(theme.border)
                            .bg(theme.tab_bar)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(theme.muted_foreground)
                                    .child(filename),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .w_full()
                            .min_h_0()
                            .overflow_hidden()
                            .child(editor),
                    )
                    .child(comment_panel)
                    .into_any_element();
            }

            if file.patch.is_none() {
                let theme = cx.theme();
                return div()
                    .debug_selector(|| "pr-diff-area".into())
                    .flex_1()
                    .h_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_sm()
                            .text_color(theme.muted_foreground)
                            .child("Binary file or too large to display"),
                    )
                    .into_any_element();
            }
        }

        let theme = cx.theme();
        div()
            .debug_selector(|| "pr-diff-area".into())
            .flex_1()
            .h_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Select a file to view diff"),
            )
            .into_any_element()
    }

    fn render_loading(&self, cx: &Context<Self>) -> AnyElement {
        let theme = cx.theme();
        div()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .child(
                div()
                    .text_sm()
                    .text_color(theme.muted_foreground)
                    .child("Loading PR details..."),
            )
            .into_any_element()
    }
}

impl Render for PrReviewView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let focus_handle = self.focus_handle.clone();
        let files = self.files(cx);
        let has_details = self
            .github_store
            .read(cx)
            .pr_details(self.pr_number)
            .is_some();

        let header = self.render_header(cx);

        let body: AnyElement = if has_details {
            let file_list = self.render_file_list(&files, cx);
            let diff_area = self.render_diff_area(&files, window, cx);
            div()
                .flex_1()
                .w_full()
                .min_h_0()
                .flex()
                .flex_row()
                .child(file_list)
                .child(diff_area)
                .into_any_element()
        } else {
            self.render_loading(cx)
        };

        let bg = cx.theme().background;

        div()
            .id("pr-review")
            .debug_selector(|| "pr-review".into())
            .key_context("PrReview")
            .track_focus(&focus_handle)
            .on_action(cx.listener(|this, _: &PrReviewNextFile, _window, cx| {
                this.next_file(cx);
            }))
            .on_action(cx.listener(|this, _: &PrReviewPrevFile, _window, cx| {
                this.prev_file(cx);
            }))
            .size_full()
            .flex()
            .flex_col()
            .bg(bg)
            .child(header)
            .child(body)
    }
}

impl Focusable for PrReviewView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Tab for PrReviewView {
    fn label(&self, _cx: &App) -> String {
        format!("PR #{}", self.pr_number)
    }

    fn to_config(&self, _cx: &App) -> TabConfig {
        TabConfig::PrReview(PrReviewTabConfig {
            owner: self.owner.clone(),
            repo: self.repo.clone(),
            pr_number: self.pr_number,
            pr_title: self.pr_title.clone(),
        })
    }
}

fn format_timestamp(timestamp: &str) -> String {
    if timestamp.len() >= 10 {
        timestamp[..10].to_string()
    } else {
        timestamp.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[core::prelude::v1::test]
    fn test_pr_review_tab_config_serialization() {
        let config = PrReviewTabConfig {
            owner: "owner".to_string(),
            repo: "repo".to_string(),
            pr_number: 42,
            pr_title: "Test PR".to_string(),
        };

        let tab_config = TabConfig::PrReview(config);
        let json = serde_json::to_string(&tab_config).unwrap();
        let deserialized: TabConfig = serde_json::from_str(&json).unwrap();

        if let TabConfig::PrReview(pr_config) = deserialized {
            assert_eq!(pr_config.pr_number, 42);
            assert_eq!(pr_config.owner, "owner");
            assert_eq!(pr_config.repo, "repo");
            assert_eq!(pr_config.pr_title, "Test PR");
        } else {
            panic!("Expected PrReview tab config");
        }
    }
}
