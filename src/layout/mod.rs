pub mod left_sidebar;
pub mod main_tabs;
pub mod right_sidebar;

pub use left_sidebar::LeftSidebarPanel;
pub use main_tabs::MainTabsPanel;
pub use right_sidebar::RightSidebarPanel;

use swrm::app_state::AppState;
use gpui::{AppContext, Context, Entity, Window};
use gpui_component::dock::{DockArea, DockItem, DockPlacement};
use std::sync::Arc;

pub fn build(
    state: Entity<AppState>,
    window: &mut Window,
    cx: &mut Context<DockArea>,
) -> DockArea {
    let mut area = DockArea::new("swrm-root", Some(1), window, cx);
    let weak = cx.entity().downgrade();

    let left = cx.new(|cx| LeftSidebarPanel::new(state.clone(), window, cx));
    let right = cx.new(|cx| RightSidebarPanel::new(state.clone(), window, cx));
    let center_tabs = cx.new(|cx| MainTabsPanel::new(state.clone(), window, cx));

    area.add_panel(Arc::new(left), DockPlacement::Left, None, window, cx);
    area.add_panel(Arc::new(right), DockPlacement::Right, None, window, cx);

    let center = DockItem::tabs(vec![Arc::new(center_tabs)], &weak, window, cx);
    area.set_center(center, window, cx);

    area
}
