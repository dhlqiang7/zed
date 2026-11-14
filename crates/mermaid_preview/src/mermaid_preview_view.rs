use std::path::Path;

use file_icons::FileIcons;
use gpui::{
    App, Context, Entity, EventEmitter, FocusHandle, Focusable, IntoElement, ParentElement, Render,
    SharedString, Styled, Subscription, WeakEntity, Window, div,
};
use language::{Buffer, BufferEvent};
use markdown::{
    CodeBlockRenderer, CopyButtonVisibility, Markdown, MarkdownElement, MarkdownFont,
    MarkdownOptions, MarkdownStyle, WrapButtonVisibility,
};
use multi_buffer::MultiBuffer;
use project::ProjectPath;
use ui::prelude::*;
use workspace::item::Item;
use workspace::notifications::NotifyResultExt;
use workspace::{Pane, Workspace};
use zed_i18n::t;

use crate::{OpenFollowingPreview, OpenPreview, OpenPreviewToTheSide};

pub struct MermaidPreviewView {
    focus_handle: FocusHandle,
    buffer: Option<Entity<Buffer>>,
    markdown: Entity<Markdown>,
    _buffer_subscription: Option<Subscription>,
    _workspace_subscription: Option<Subscription>,
    _theme_subscription: Subscription,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum MermaidPreviewMode {
    /// The preview will always show the contents of the provided editor.
    Default,
    /// The preview will "follow" the last active editor of a Mermaid file.
    Follow,
}

/// Wraps raw diagram source in a fenced `mermaid` block, so the preview
/// reuses the markdown crate's rendering path (theme, zoom, pan, code tab).
fn mermaid_fenced_source(text: &str) -> SharedString {
    format!("```mermaid\n{}\n```", text.trim_end()).into()
}

impl MermaidPreviewView {
    pub fn new(
        mode: MermaidPreviewMode,
        active_buffer: Entity<MultiBuffer>,
        workspace_handle: WeakEntity<Workspace>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Entity<Self> {
        cx.new(|cx| {
            let workspace_subscription = if mode == MermaidPreviewMode::Follow
                && let Some(workspace) = workspace_handle.upgrade()
            {
                Some(Self::subscribe_to_workspace(workspace, window, cx))
            } else {
                None
            };

            let buffer = active_buffer.read_with(cx, |buffer, _cx| buffer.as_singleton());

            let subscription = buffer
                .as_ref()
                .map(|buffer| Self::create_buffer_subscription(buffer, window, cx));

            let markdown = cx.new(|cx| {
                let source = buffer
                    .as_ref()
                    .map(|buffer| mermaid_fenced_source(&buffer.read(cx).snapshot().text()))
                    .unwrap_or_default();
                Markdown::new_with_options(
                    source,
                    None,
                    None,
                    MarkdownOptions {
                        render_mermaid_diagrams: true,
                        ..Default::default()
                    },
                    cx,
                )
            });

            let theme_subscription =
                cx.observe_global::<theme::GlobalTheme>(|_, cx| cx.notify());

            Self {
                focus_handle: cx.focus_handle(),
                buffer,
                markdown,
                _buffer_subscription: subscription,
                _workspace_subscription: workspace_subscription,
                _theme_subscription: theme_subscription,
            }
        })
    }

    fn subscribe_to_workspace(
        workspace: Entity<Workspace>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            &workspace,
            window,
            move |this: &mut MermaidPreviewView,
                    workspace,
                    event: &workspace::Event,
                    window,
                    cx| {
                if let workspace::Event::ActiveItemChanged = event {
                    let workspace = workspace.read(cx);
                    if let Some(active_item) = workspace.active_item(cx)
                        && let Some(buffer) = active_item.downcast::<MultiBuffer>()
                        && Self::is_mermaid_file(&buffer, cx)
                    {
                        let Some(buffer) = buffer.read(cx).as_singleton() else {
                            return;
                        };
                        if this.buffer.as_ref() != Some(&buffer) {
                            this._buffer_subscription =
                                Some(Self::create_buffer_subscription(&buffer, window, cx));
                            this.buffer = Some(buffer.clone());
                            this.set_markdown_source(&buffer, cx);
                            cx.notify();
                        }
                    }
                }
            },
        )
    }

    fn create_buffer_subscription(
        buffer: &Entity<Buffer>,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Subscription {
        cx.subscribe_in(
            buffer,
            window,
            move |this, buffer, event: &BufferEvent, _window, cx| match event {
                BufferEvent::Edited { .. } | BufferEvent::Saved => {
                    this.set_markdown_source(buffer, cx);
                }
                _ => {}
            },
        )
    }

    fn set_markdown_source(&mut self, buffer: &Entity<Buffer>, cx: &mut Context<Self>) {
        let source = mermaid_fenced_source(&buffer.read(cx).snapshot().text());
        self.markdown.update(cx, |markdown, cx| {
            markdown.reset(source, cx);
        });
        cx.notify();
    }

    pub fn open_for_project_path(
        project_path: ProjectPath,
        workspace: &mut Workspace,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        let open_buffer = workspace
            .project()
            .update(cx, |project, cx| project.open_buffer(project_path, cx));

        cx.spawn_in(window, async move |workspace, cx| {
            let Some(buffer) = open_buffer
                .await
                .notify_workspace_async_err(workspace.clone(), cx)
            else {
                return;
            };

            workspace
                .update_in(cx, |workspace, window, cx| {
                    let buffer = cx.new(|cx| MultiBuffer::singleton(buffer, cx));
                    let pane = workspace.active_pane().clone();
                    Self::open_preview_in_pane(workspace, buffer, pane, window, cx);
                })
                .ok();
        })
        .detach();
    }

    pub fn is_mermaid_path(path: &Path) -> bool {
        path.extension().is_some_and(|extension| {
            extension.eq_ignore_ascii_case("mmd") || extension.eq_ignore_ascii_case("mermaid")
        })
    }

    pub fn is_mermaid_file(buffer: &Entity<MultiBuffer>, cx: &App) -> bool {
        buffer
            .read(cx)
            .as_singleton()
            .and_then(|buffer| buffer.read(cx).file())
            .is_some_and(|file| Self::is_mermaid_path(Path::new(file.file_name(cx))))
    }

    pub fn resolve_active_item_as_mermaid_buffer(
        workspace: &Workspace,
        cx: &mut Context<Workspace>,
    ) -> Option<Entity<MultiBuffer>> {
        workspace
            .active_item(cx)?
            .act_as::<MultiBuffer>(cx)
            .filter(|buffer| Self::is_mermaid_file(buffer, cx))
    }

    fn find_existing_preview_item_idx(
        pane: &Pane,
        buffer: &Entity<MultiBuffer>,
        cx: &App,
    ) -> Option<usize> {
        let buffer_id = buffer.read(cx).as_singleton()?.entity_id();
        pane.items_of_type::<MermaidPreviewView>()
            .find(|view| {
                view.read(cx)
                    .buffer
                    .as_ref()
                    .is_some_and(|buffer| buffer.entity_id() == buffer_id)
            })
            .and_then(|view| pane.index_for_item(&view))
    }

    fn create_preview_view(
        mode: MermaidPreviewMode,
        workspace: &mut Workspace,
        buffer: Entity<MultiBuffer>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) -> Entity<MermaidPreviewView> {
        let workspace_handle = workspace.weak_handle();
        MermaidPreviewView::new(mode, buffer, workspace_handle, window, cx)
    }

    pub fn open_preview_in_pane(
        workspace: &mut Workspace,
        buffer: Entity<MultiBuffer>,
        pane: Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        Self::activate_or_add_preview(workspace, buffer, pane, true, window, cx);
    }

    pub fn open_preview_to_the_side_of_pane(
        workspace: &mut Workspace,
        buffer: Entity<MultiBuffer>,
        origin_pane: Entity<Pane>,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        let target_pane = workspace.adjacent_pane_of(&origin_pane, window, cx);
        Self::activate_or_add_preview(workspace, buffer, target_pane, false, window, cx);
    }

    fn activate_or_add_preview(
        workspace: &mut Workspace,
        buffer: Entity<MultiBuffer>,
        pane: Entity<Pane>,
        focus: bool,
        window: &mut Window,
        cx: &mut Context<Workspace>,
    ) {
        let existing_view_idx = Self::find_existing_preview_item_idx(pane.read(cx), &buffer, cx);
        if let Some(existing_view_idx) = existing_view_idx {
            pane.update(cx, |pane, cx| {
                pane.activate_item(existing_view_idx, focus, focus, window, cx);
            });
        } else {
            let view = Self::create_preview_view(
                MermaidPreviewMode::Default,
                workspace,
                buffer,
                window,
                cx,
            );
            pane.update(cx, |pane, cx| {
                pane.add_item(Box::new(view), focus, focus, None, window, cx)
            });
        }
        cx.notify();
    }

    pub fn register(workspace: &mut Workspace, _window: &mut Window, _cx: &mut Context<Workspace>) {
        workspace.register_action(move |workspace, _: &OpenPreview, window, cx| {
            if let Some(item) = workspace.item_for_action(window, cx)
                && let Some(buffer) = item.act_as::<MultiBuffer>(cx)
                && Self::is_mermaid_file(&buffer, cx)
                && let Some(pane) = workspace.pane_for_item_id(item.item_id())
            {
                Self::open_preview_in_pane(workspace, buffer, pane, window, cx);
            }
        });

        workspace.register_action(move |workspace, _: &OpenPreviewToTheSide, window, cx| {
            if let Some(buffer) = Self::resolve_active_item_as_mermaid_buffer(workspace, cx) {
                let pane = workspace.active_pane().clone();
                Self::open_preview_to_the_side_of_pane(workspace, buffer, pane, window, cx);
            }
        });

        workspace.register_action(move |workspace, _: &OpenFollowingPreview, window, cx| {
            if let Some(editor) = Self::resolve_active_item_as_mermaid_buffer(workspace, cx) {
                let view =
                    Self::create_preview_view(MermaidPreviewMode::Follow, workspace, editor, window, cx);
                workspace.active_pane().update(cx, |pane, cx| {
                    pane.add_item(Box::new(view), true, true, None, window, cx)
                });
                cx.notify();
            }
        });
    }
}

impl Render for MermaidPreviewView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let markdown_style = MarkdownStyle::themed(MarkdownFont::Preview, window, cx);
        let markdown_element = MarkdownElement::new(self.markdown.clone(), markdown_style)
            .input_focus_handle(self.focus_handle.clone())
            .code_block_renderer(CodeBlockRenderer::Default {
                copy_button_visibility: CopyButtonVisibility::VisibleOnHover,
                wrap_button_visibility: WrapButtonVisibility::Hidden,
                border: false,
            });

        v_flex()
            .id("MermaidPreview")
            .key_context("MermaidPreview")
            .track_focus(&self.focus_handle(cx))
            .size_full()
            .overflow_y_scroll()
            .bg(cx.theme().colors().editor_background)
            .child(div().p_4().child(markdown_element))
    }
}

impl Focusable for MermaidPreviewView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl EventEmitter<()> for MermaidPreviewView {}

impl Item for MermaidPreviewView {
    type Event = ();

    fn tab_icon(&self, _window: &Window, cx: &App) -> Option<Icon> {
        self.buffer
            .as_ref()
            .and_then(|buffer| buffer.read(cx).file())
            .and_then(|file| FileIcons::get_icon(file.path().as_std_path(), cx))
            .map(Icon::from_path)
            .or_else(|| Some(Icon::new(IconName::Image)))
    }

    fn tab_content_text(&self, _detail: usize, cx: &App) -> SharedString {
        self.buffer
            .as_ref()
            .and_then(|mermaid_path| mermaid_path.read(cx).file())
            .map(|name| t!("mermaid_preview.preview_name", name = name.file_name(cx)).into())
            .unwrap_or_else(|| t!("mermaid_preview.tab_title").to_string().into())
    }

    fn telemetry_event_text(&self) -> Option<&'static str> {
        Some("mermaid preview: open")
    }

    fn to_item_events(_event: &Self::Event, _f: &mut dyn FnMut(workspace::item::ItemEvent)) {}
}
