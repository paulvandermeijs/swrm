mod app;
mod layout;

use anyhow::Result;
use gpui::{App, AppContext, Bounds, WindowBounds, WindowOptions, px, size};
use gpui_component::{Theme, ThemeMode, TitleBar};
use std::borrow::Cow;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info,swrm=debug")),
        )
        .init();

    gpui_platform::application()
        .with_assets(gpui_component_assets::Assets)
        .run(|cx: &mut App| {
            gpui_component::init(cx);
            Theme::change(ThemeMode::Dark, None, cx);

            // Bundle open-source fonts: IBM Plex Sans for UI, JetBrains Mono for the terminal.
            let fonts: Vec<Cow<'static, [u8]>> = vec![
                Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexSans-Regular.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/IBMPlexSans-SemiBold.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf")),
                Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-BoldItalic.ttf")),
            ];
            cx.text_system()
                .add_fonts(fonts)
                .expect("load bundled fonts");
            cx.global_mut::<Theme>().font_family = "IBM Plex Sans".into();
            cx.global_mut::<Theme>().mono_font_family = "JetBrains Mono".into();

            cx.bind_keys([
                gpui::KeyBinding::new("cmd-b", app::ToggleLeftSidebar, Some("Root")),
                gpui::KeyBinding::new("cmd-l", app::ToggleRightSidebar, Some("Root")),
                gpui::KeyBinding::new("cmd-t", app::NewTab, Some("Root")),
                gpui::KeyBinding::new("cmd-w", app::CloseTab, Some("Root")),
                gpui::KeyBinding::new("cmd-1", app::SelectTab1, Some("Root")),
                gpui::KeyBinding::new("cmd-2", app::SelectTab2, Some("Root")),
                gpui::KeyBinding::new("cmd-3", app::SelectTab3, Some("Root")),
                gpui::KeyBinding::new("cmd-4", app::SelectTab4, Some("Root")),
                gpui::KeyBinding::new("cmd-5", app::SelectTab5, Some("Root")),
                gpui::KeyBinding::new("cmd-6", app::SelectTab6, Some("Root")),
                gpui::KeyBinding::new("cmd-7", app::SelectTab7, Some("Root")),
                gpui::KeyBinding::new("cmd-8", app::SelectTab8, Some("Root")),
                gpui::KeyBinding::new("cmd-9", app::SelectTab9, Some("Root")),
            ]);
            let bounds = Bounds::centered(None, size(px(1280.), px(800.)), cx);
            cx.open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(bounds)),
                    titlebar: Some(TitleBar::title_bar_options()),
                    ..Default::default()
                },
                |window, cx| {
                    let app_root = cx.new(|cx| app::Root::new(window, cx));
                    cx.new(|cx| gpui_component::Root::new(app_root, window, cx))
                },
            )
            .expect("failed to open window");
            cx.activate(true);
        });
    Ok(())
}
