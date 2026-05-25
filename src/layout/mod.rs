pub mod left_sidebar;
pub mod main_tabs;
pub mod right_sidebar;
pub mod settings_view;

pub use left_sidebar::LeftSidebarPanel;
pub use main_tabs::MainTabsPanel;
pub use right_sidebar::RightSidebarPanel;

use gpui::{AppContext, Entity, Window};
use gpui_component::dock::{DockArea, DockItem, DockPlacement};
use std::sync::Arc;
use swrm::app_state::AppState;

pub struct Layout {
    pub dock: Entity<DockArea>,
    pub left: Entity<LeftSidebarPanel>,
    pub right: Entity<RightSidebarPanel>,
    pub tabs: Entity<MainTabsPanel>,
}

pub fn build<T: 'static>(
    state: Entity<AppState>,
    window: &mut Window,
    cx: &mut gpui::Context<T>,
) -> Layout {
    let left = cx.new(|cx| LeftSidebarPanel::new(state.clone(), window, cx));
    let right = cx.new(|cx| RightSidebarPanel::new(state.clone(), window, cx));
    let tabs = cx.new(|cx| MainTabsPanel::new(state.clone(), window, cx));

    let left_arc = Arc::new(left.clone());
    let right_arc = Arc::new(right.clone());
    let tabs_arc = Arc::new(tabs.clone());

    let dock = cx.new(|cx| {
        let mut area = DockArea::new("swrm-root", Some(1), window, cx);
        let weak = cx.entity().downgrade();

        area.add_panel(left_arc, DockPlacement::Left, None, window, cx);
        area.add_panel(right_arc, DockPlacement::Right, None, window, cx);

        let center = DockItem::tabs(vec![tabs_arc], &weak, window, cx);
        area.set_center(center, window, cx);

        area
    });

    Layout {
        dock,
        left,
        right,
        tabs,
    }
}
