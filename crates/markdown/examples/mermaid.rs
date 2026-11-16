//! Interactive playground for the mermaid rendering pipeline.
//!
//! Run with:
//!
//! ```bash
//! RUSTUP_TOOLCHAIN=zed-1.98.1 cargo run -p markdown --example mermaid -j 1
//! ```
//!
//! This exercises the exact production path shared by the markdown preview
//! and the `.mmd` preview pane — zoom (ctrl/cmd + wheel, up to 400%), drag to
//! pan, the code/preview tab, and theme-aware rendering — without building
//! the whole editor.

use gpui::{AppContext, Application, Bounds, Entity, WindowBounds, WindowOptions, point, px, size};
use markdown::{Markdown, MarkdownElement, MarkdownFont, MarkdownOptions, MarkdownStyle};
use ui::prelude::*;

const DEMO: &str = r#"
# Mermaid playground

Ctrl/⌘ + 滚轮缩放（最大 400%）；左键按住拖动平移；右上角可切换 代码/预览。
宽图用于验证放大后尺寸不回缩。

```mermaid
flowchart LR
    A[Playground] --> B{Zoom held at 400%?}
    B -- yes --> C[Ship it]
    B -- no --> D[File a bug]
```

```mermaid
flowchart LR
    W1 --> W2 --> W3 --> W4 --> W5 --> W6 --> W7 --> W8 --> W9 --> W10
```

```mermaid
sankey-beta

a,b,10
b,c,5
a,c,3
```
"#;

struct Playground {
    markdown: Entity<Markdown>,
}

impl Render for Playground {
    fn render(
        &mut self,
        window: &mut gpui::Window,
        cx: &mut gpui::Context<Self>,
    ) -> impl IntoElement {
        let style = MarkdownStyle::themed(MarkdownFont::Preview, window, cx);
        div()
            .id("mermaid-playground")
            .size_full()
            .bg(cx.theme().colors().editor_background)
            .overflow_y_scroll()
            .p_6()
            .child(MarkdownElement::new(self.markdown.clone(), style))
    }
}

fn main() {
    gpui_platform::application().run(|cx| {
        settings::init(cx);
        theme_settings::init(theme::LoadThemes::All(Box::new(assets::Assets)), cx);

        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds {
                origin: point(px(150.), px(150.)),
                size: size(px(1100.), px(850.)),
            })),
            ..Default::default()
        };
        cx.open_window(options, |_, cx| {
            cx.new(|cx| {
                let markdown = cx.new(|cx| {
                    Markdown::new_with_options(
                        DEMO.into(),
                        None,
                        None,
                        MarkdownOptions {
                            render_mermaid_diagrams: true,
                            ..Default::default()
                        },
                        cx,
                    )
                });
                Playground { markdown }
            })
        })
        .unwrap();
        cx.activate(true);
    });
}
