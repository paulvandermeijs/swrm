mod app;
mod app_state;
mod layout;

use anyhow::Result;
use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,swrm=debug")),
        )
        .init();

    gpui_platform::application().run(|cx: &mut App| {
        gpui_component::init(cx);
        cx.bind_keys([
            gpui::KeyBinding::new("cmd-b", app::ToggleLeftSidebar, Some("Root")),
            gpui::KeyBinding::new("cmd-l", app::ToggleRightSidebar, Some("Root")),
        ]);
        let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                titlebar: Some(gpui::TitlebarOptions {
                    title: Some("swrm".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            |window, cx| cx.new(|cx| app::Root::new(window, cx)),
        )
        .expect("failed to open window");
        cx.activate(true);
    });
    Ok(())
}
