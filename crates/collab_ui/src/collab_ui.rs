// clean-cn: 协作子模块 init 已注释，私有模块需 allow 防止 dead_code 告警
#[allow(dead_code)]
mod call_stats_modal;
pub mod channel_view;
pub mod collab_panel;
pub mod notifications;
mod panel_settings;

use std::{rc::Rc, sync::Arc};

pub use collab_panel::CollabPanel;
use gpui::{
    App, Pixels, PlatformDisplay, Size, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowKind, WindowOptions, point,
};
pub use panel_settings::CollaborationPanelSettings;
use release_channel::ReleaseChannel;
use ui::px;
use workspace::AppState;

// Another comment, nice.
pub fn init(_app_state: &Arc<AppState>, cx: &mut App) {
    // clean-cn: 协作功能（通话统计/频道/协作面板/通知）依赖 zed.dev 账号，注册移除；
    // title_bar::init 是全局标题栏组件初始化，与协作无关，必须保留
    // call_stats_modal::init(cx);
    // channel_view::init(cx);
    // collab_panel::init(cx);
    // notifications::init(app_state, cx);
    title_bar::init(cx);
}

fn notification_window_options(
    screen: Rc<dyn PlatformDisplay>,
    size: Size<Pixels>,
    cx: &App,
) -> WindowOptions {
    let notification_margin_width = px(16.);
    let notification_margin_height = px(-48.);

    let bounds = gpui::Bounds::<Pixels> {
        origin: screen.bounds().top_right()
            - point(
                size.width + notification_margin_width,
                notification_margin_height,
            ),
        size,
    };

    let app_id = ReleaseChannel::global(cx).app_id();

    WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(bounds)),
        titlebar: None,
        focus: false,
        show: true,
        kind: WindowKind::PopUp,
        is_movable: false,
        display_id: Some(screen.id()),
        window_background: WindowBackgroundAppearance::Transparent,
        app_id: Some(app_id.to_owned()),
        window_min_size: None,
        window_decorations: Some(WindowDecorations::Client),
        tabbing_identifier: None,
        ..Default::default()
    }
}
