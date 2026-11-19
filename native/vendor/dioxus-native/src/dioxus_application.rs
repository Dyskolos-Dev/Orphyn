use blitz_shell::{BlitzApplication, BlitzShellProxy, View};
use dioxus_core::{ScopeId, provide_context};
use dioxus_history::{History, MemoryHistory};
use std::rc::Rc;
use std::sync::Arc;
use winit::application::ApplicationHandler;
use winit::event::{StartCause, WindowEvent};
use winit::event_loop::ActiveEventLoop;
use winit::window::WindowId;

#[cfg(target_os = "macos")]
use winit::platform::macos::ApplicationHandlerExtMacOS;

use blitz_dom::DocumentConfig;

use crate::{DioxusNativeWindowRenderer, RendererOptions};
use crate::event_handlers::WindowEventHandlers;
use crate::{BlitzShellEvent, DioxusDocument, WindowConfig, contexts::DioxusNativeDocument};

/// Orphyn patch (capture mode): called with the main window's document after every event batch.
pub type FrameHook = Box<dyn FnMut(&mut blitz_dom::BaseDocument, &dyn ActiveEventLoop) -> bool>;

thread_local! {
    static FRAME_HOOK: std::cell::RefCell<Option<FrameHook>> = const { std::cell::RefCell::new(None) };
}

/// Orphyn patch: registers the hook (capture mode). The loop then wakes every 100 ms, at once
/// again when the hook returns true.
pub fn set_frame_hook(hook: FrameHook) {
    FRAME_HOOK.with(|h| *h.borrow_mut() = Some(hook));
}

/// Dioxus-native specific event type
pub enum DioxusNativeEvent {
    /// A hotreload event, basically telling us to update our templates.
    #[cfg(all(feature = "hot-reload", debug_assertions))]
    DevserverEvent(dioxus_devtools::DevserverMsg),

    /// Create a new head element from the Link and Title elements
    ///
    /// todo(jon): these should probabkly be synchronous somehow
    /// Orphyn patch: open the extra window registered under `ticket`.
    OpenWindow { ticket: u64 },

    /// Orphyn patch: close the extra window opened under `ticket`.
    CloseWindow { ticket: u64 },

    CreateHeadElement {
        window: WindowId,
        name: String,
        attributes: Vec<(String, String)>,
        contents: Option<String>,
    },
}

/// Orphyn patch: what extra windows need to be built like the first one.
pub struct ExtraWindowEnv {
    pub net_provider: Arc<dyn blitz_traits::net::NetProvider>,
    #[cfg(all(feature = "net", not(target_arch = "wasm32")))]
    pub inner_net: Arc<blitz_net::Provider>,
    pub html_parser: Option<Arc<dyn blitz_dom::HtmlParserProvider>>,
    pub navigation: Option<Arc<dyn blitz_traits::navigation::NavigationProvider>>,
    pub font_ctx: Option<blitz_dom::FontContext>,
    pub base_color: Option<peniko::Color>,
    pub alpha_mode: Option<anyrender::CompositeAlphaMode>,
    #[cfg(any(feature = "vello", feature = "vello-hybrid"))]
    pub features: Option<crate::Features>,
    #[cfg(any(feature = "vello", feature = "vello-hybrid"))]
    pub limits: Option<crate::Limits>,
}

pub struct DioxusNativeApplication {
    env: Option<ExtraWindowEnv>,
    main_window: Option<WindowId>,
    /// Extra windows by ticket: the window and what to run once it is gone.
    extras: std::collections::HashMap<u64, (WindowId, Box<dyn FnOnce()>)>,
    pending_window: Option<WindowConfig<DioxusNativeWindowRenderer>>,
    inner: BlitzApplication<DioxusNativeWindowRenderer>,
    event_handlers: Rc<WindowEventHandlers>,
}

impl DioxusNativeApplication {
    pub fn new(
        proxy: BlitzShellProxy,
        event_queue: std::sync::mpsc::Receiver<BlitzShellEvent>,
        config: WindowConfig<DioxusNativeWindowRenderer>,
    ) -> Self {
        Self {
            env: None,
            main_window: None,
            extras: Default::default(),
            pending_window: Some(config),
            inner: BlitzApplication::new(proxy, event_queue),
            event_handlers: Rc::new(WindowEventHandlers::default()),
        }
    }

    /// Orphyn patch: lets [`ExtraWindows`](crate::ExtraWindows) build windows.
    pub fn with_extra_windows(mut self, env: ExtraWindowEnv) -> Self {
        self.env = Some(env);
        self
    }

    /// Orphyn patch: runs what a closed extra window's opener asked for, in the main window.
    fn run_on_close(&mut self, on_close: Box<dyn FnOnce()>) {
        match self.main_window.and_then(|id| self.inner.windows.get_mut(&id)) {
            Some(main) => {
                main.downcast_doc_mut::<DioxusDocument>()
                    .vdom
                    .in_scope(ScopeId::ROOT, on_close);
                main.poll();
            }
            None => on_close(),
        }
    }

    fn run_frame_hook(&mut self, event_loop: &dyn ActiveEventLoop) {
        if !FRAME_HOOK.with(|h| h.borrow().is_some()) {
            return;
        }
        let mut busy = false;
        if let Some(main) = self.main_window.and_then(|id| self.inner.windows.get_mut(&id)) {
            main.poll();
            let doc = main.downcast_doc_mut::<DioxusDocument>();
            FRAME_HOOK.with(|h| {
                if let Some(hook) = h.borrow_mut().as_mut() {
                    busy = hook(&mut doc.inner.borrow_mut(), event_loop);
                }
            });
        }
        let wait = if busy { 1 } else { 100 };
        event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
            std::time::Instant::now() + std::time::Duration::from_millis(wait),
        ));
    }

    fn ticket_of(&self, window: WindowId) -> Option<u64> {
        self.extras
            .iter()
            .find_map(|(t, (id, _))| (*id == window).then_some(*t))
    }

    fn close_extra(&mut self, ticket: u64) {
        if let Some((id, on_close)) = self.extras.remove(&ticket) {
            drop(self.inner.windows.remove(&id));
            self.run_on_close(on_close);
        }
    }

    fn open_extra(&mut self, event_loop: &dyn ActiveEventLoop, ticket: u64) {
        let Some(p) = crate::extra_windows::PENDING.with(|p| p.borrow_mut().remove(&ticket))
        else {
            return;
        };
        let Some(env) = self.env.as_ref() else {
            return;
        };
        let vdom = (p.build)();
        #[cfg(all(feature = "net", not(target_arch = "wasm32")))]
        vdom.provide_root_context(Arc::clone(&env.inner_net));
        vdom.provide_root_context(Arc::clone(&env.net_provider));
        if let Some(h) = &env.html_parser {
            vdom.provide_root_context(Arc::clone(h));
        }
        let doc = DioxusDocument::new(
            vdom,
            DocumentConfig {
                net_provider: Some(Arc::clone(&env.net_provider)),
                html_parser_provider: env.html_parser.clone(),
                navigation_provider: env.navigation.clone(),
                font_ctx: env.font_ctx.clone(),
                ..Default::default()
            },
        );
        // A transparent window (rounded card, no frame) clears to transparent and composites alpha.
        let transparent = p.attributes.transparent;
        let options = RendererOptions {
            base_color: if transparent { Some(peniko::Color::TRANSPARENT) } else { env.base_color },
            alpha_mode: if transparent {
                Some(anyrender::CompositeAlphaMode::Transparent)
            } else {
                env.alpha_mode
            },
            #[cfg(any(feature = "vello", feature = "vello-hybrid"))]
            features: env.features.clone(),
            #[cfg(any(feature = "vello", feature = "vello-hybrid"))]
            limits: env.limits.clone(),
        };
        let config = WindowConfig::with_attributes(
            Box::new(doc) as _,
            DioxusNativeWindowRenderer::with_options(options),
            p.attributes,
        );
        let id = self.init_window(config, event_loop);
        // The main window is resumed by `can_create_surfaces`; a window opened later has to
        // create its render surface itself, or it stays blank (the invisible PiP).
        if let Some(view) = self.inner.windows.get_mut(&id) {
            view.resume();
        }
        self.extras.insert(ticket, (id, p.on_close));
    }

    /// Creates the window and provides the contexts every window gets.
    fn init_window(
        &mut self,
        config: WindowConfig<DioxusNativeWindowRenderer>,
        event_loop: &dyn ActiveEventLoop,
    ) -> WindowId {
        let mut window = View::init(config, event_loop, &self.inner.proxy);
        let winit_window = Arc::clone(&window.window);
        let renderer = window.renderer.clone();
        let window_id = window.window_id();
        let doc = window.downcast_doc_mut::<DioxusDocument>();

        doc.vdom.in_scope(ScopeId::ROOT, || {
            let shared: Rc<dyn dioxus_document::Document> = Rc::new(DioxusNativeDocument::new(
                self.inner.proxy.clone(),
                window_id,
            ));
            provide_context(shared);
            provide_context(self.event_handlers.clone());
            provide_context(crate::ExtraWindows {
                proxy: self.inner.proxy.clone(),
            });
        });
        let shell_provider = doc.inner.borrow().shell_provider.clone();
        doc.vdom
            .in_scope(ScopeId::ROOT, move || provide_context(shell_provider));
        let history_provider: Rc<dyn History> = Rc::new(MemoryHistory::default());
        doc.vdom
            .in_scope(ScopeId::ROOT, move || provide_context(history_provider));
        doc.vdom
            .in_scope(ScopeId::ROOT, move || provide_context(renderer));
        doc.vdom
            .in_scope(ScopeId::ROOT, move || provide_context(winit_window));
        doc.initial_build();
        window.request_redraw();
        self.inner.windows.insert(window_id, window);
        window_id
    }

    pub fn add_window(&mut self, window_config: WindowConfig<DioxusNativeWindowRenderer>) {
        self.inner.add_window(window_config);
    }

    fn handle_dioxus_native_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        event: &DioxusNativeEvent,
    ) {
        match event {
            #[cfg(all(feature = "hot-reload", debug_assertions))]
            DioxusNativeEvent::DevserverEvent(event) => match event {
                dioxus_devtools::DevserverMsg::HotReload(hotreload_message) => {
                    for window in self.inner.windows.values_mut() {
                        let doc = window.downcast_doc_mut::<DioxusDocument>();

                        // Apply changes to vdom
                        dioxus_devtools::apply_changes(&doc.vdom, hotreload_message);

                        // Reload changed assets
                        for asset_path in &hotreload_message.assets {
                            if let Some(url) = asset_path.to_str() {
                                doc.inner.borrow_mut().reload_resource_by_href(url);
                            }
                        }

                        window.poll();
                    }
                }
                dioxus_devtools::DevserverMsg::Shutdown => event_loop.exit(),
                dioxus_devtools::DevserverMsg::FullReloadStart => {}
                dioxus_devtools::DevserverMsg::FullReloadFailed => {}
                dioxus_devtools::DevserverMsg::FullReloadCommand => {}
                _ => {}
            },

            DioxusNativeEvent::OpenWindow { ticket } => self.open_extra(event_loop, *ticket),
            DioxusNativeEvent::CloseWindow { ticket } => self.close_extra(*ticket),

            DioxusNativeEvent::CreateHeadElement {
                name,
                attributes,
                contents,
                window,
            } => {
                if let Some(window) = self.inner.windows.get_mut(window) {
                    let doc = window.downcast_doc_mut::<DioxusDocument>();
                    doc.create_head_element(name, attributes, contents);
                    window.poll();
                }
            }

            // Suppress unused variable warning
            #[cfg(not(all(feature = "hot-reload", debug_assertions)))]
            #[allow(unreachable_patterns)]
            _ => {
                let _ = event_loop;
                let _ = event;
            }
        }
    }
}

impl ApplicationHandler for DioxusNativeApplication {
    #[cfg(target_os = "macos")]
    fn macos_handler(&mut self) -> Option<&mut dyn ApplicationHandlerExtMacOS> {
        self.inner.macos_handler()
    }

    fn resumed(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.resumed(event_loop);
    }

    fn suspended(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.suspended(event_loop);
    }

    fn destroy_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.destroy_surfaces(event_loop);
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        self.inner.about_to_wait(event_loop);
        self.run_frame_hook(event_loop);
    }

    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        #[cfg(feature = "tracing")]
        tracing::debug!("Injecting document provider into all windows");

        if let Some(config) = self.pending_window.take() {
            self.main_window = Some(self.init_window(config, event_loop));
        }

        self.inner.can_create_surfaces(event_loop);
    }

    fn new_events(&mut self, event_loop: &dyn ActiveEventLoop, cause: StartCause) {
        self.inner.new_events(event_loop, cause);
    }

    fn window_event(
        &mut self,
        event_loop: &dyn ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        self.event_handlers
            .apply_event(window_id, &event, event_loop);
        if matches!(event, WindowEvent::CloseRequested) {
            if Some(window_id) == self.main_window {
                // Closing the main window ends the app, extra windows included.
                for (_, (id, _)) in self.extras.drain() {
                    drop(self.inner.windows.remove(&id));
                }
            } else if let Some(ticket) = self.ticket_of(window_id) {
                self.close_extra(ticket);
                return;
            }
        }
        self.inner.window_event(event_loop, window_id, event);
    }

    fn proxy_wake_up(&mut self, event_loop: &dyn ActiveEventLoop) {
        while let Ok(event) = self.inner.event_queue.try_recv() {
            match event {
                BlitzShellEvent::Embedder(event) => {
                    if let Some(event) = event.downcast_ref::<DioxusNativeEvent>() {
                        self.handle_dioxus_native_event(event_loop, event);
                    }
                }
                BlitzShellEvent::CloseWindow { window_id }
                    if self.ticket_of(window_id).is_some() =>
                {
                    if let Some(ticket) = self.ticket_of(window_id) {
                        self.close_extra(ticket);
                    }
                }
                event => self.inner.handle_blitz_shell_event(event_loop, event),
            }
        }
    }
}
