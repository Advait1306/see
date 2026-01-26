use gpui::*;

use super::Axis;

/// Data passed during divider drag operations for pane resizing
#[derive(Clone)]
pub struct DividerDrag {
    pub axis: Axis,
    pub divider_index: usize,
    pub axis_path: Vec<usize>,
    pub container_size: f32,
}

impl DividerDrag {
    pub fn new(axis: Axis, divider_index: usize, axis_path: Vec<usize>, container_size: f32) -> Self {
        Self {
            axis,
            divider_index,
            axis_path,
            container_size,
        }
    }
}

impl Render for DividerDrag {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
    }
}
