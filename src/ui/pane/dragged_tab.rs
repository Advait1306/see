use gpui::{Context, Entity, IntoElement, ParentElement, Render, Styled, Window, div};
use gpui_component::theme::ActiveTheme;

use super::{Pane, TabItem};

#[derive(Clone)]
pub struct DraggedTab {
    pub pane: Entity<Pane>,
    pub tab: TabItem,
    pub index: usize,
}

impl Render for DraggedTab {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let label = self.tab.label(cx);
        let theme = cx.theme();
        div()
            .px_3()
            .py_1()
            .bg(theme.border)
            .border_1()
            .border_color(theme.list_active)
            .rounded_md()
            .text_color(theme.foreground)
            .text_xs()
            .child(label)
    }
}
