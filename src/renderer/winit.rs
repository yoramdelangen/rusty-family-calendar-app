use std::{
    num::NonZeroU32,
    rc::Rc,
    time::{Duration, Instant},
};

use softbuffer::{Context, Surface};
use taffy::{Size, prelude::length};
use winit::dpi::{LogicalSize, PhysicalSize};
use winit::platform::macos::WindowAttributesExtMacOS;
use winit::{
    application::ApplicationHandler,
    event::{ElementState, MouseButton, StartCause, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle},
    window::{Window, WindowId},
};

use crate::AppLayout;
use crate::event::AppEvent;

type Winnie = Rc<Window>;

enum AppState {
    Init,
    Suspended {
        window: Winnie,
    },
    Active {
        surface: Surface<OwnedDisplayHandle, Winnie>,
        cursor_pos: Option<(f32, f32)>,
    },
}

pub(crate) struct WinitWindowRenderer {
    context: Context<OwnedDisplayHandle>,
    state: AppState,
    layout: AppLayout,
    next_tick: Instant,
}

impl ApplicationHandler for WinitWindowRenderer {
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let mut should_render = false;
        while now >= self.next_tick {
            self.layout.handle_event(AppEvent::Tick);
            should_render = true;
            self.next_tick += Duration::from_secs(1);
        }

        if should_render {
            if let AppState::Active { surface, .. } = &mut self.state {
                Self::render_and_present(&mut self.layout, surface);
            }
        }

        event_loop.set_control_flow(ControlFlow::WaitUntil(self.next_tick));
    }

    fn new_events(&mut self, event_loop: &ActiveEventLoop, cause: StartCause) {
        if let StartCause::Init = cause {
            let window_attrs = Window::default_attributes()
                .with_title("Rusty Calendar Pi")
                .with_title_hidden(true)
                .with_fullsize_content_view(true)
                .with_titlebar_transparent(true)
                .with_inner_size(PhysicalSize::new(1920, 1080))
                .with_theme(Some(winit::window::Theme::Light));

            let window = event_loop
                .create_window(window_attrs)
                .expect("failed creating window");

            self.state = AppState::Suspended {
                window: Rc::new(window),
            };
        }
    }

    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        let AppState::Suspended { window } = &mut self.state else {
            unreachable!("got resumed event while not suspended");
        };

        let mut surface =
            Surface::new(&self.context, window.clone()).expect("cannot create surface");

        let size = window.inner_size();
        surface
            .resize(
                NonZeroU32::new(size.width).unwrap(),
                NonZeroU32::new(size.height).unwrap(),
            )
            .expect("failed to resize surface");

        let win = surface.window();
        if !win.is_visible().unwrap_or(false) {
            win.set_visible(true);
        }

        Self::render_and_present(&mut self.layout, &mut surface);
        self.state = AppState::Active {
            surface,
            cursor_pos: None,
        };
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        let AppState::Active { surface, .. } = &mut self.state else {
            unreachable!("got suspended event while not active");
        };

        let window = surface.window().clone();
        self.state = AppState::Suspended { window };
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let mut redraw = false;
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let AppState::Active { surface, .. } = &mut self.state {
                    surface
                        .resize(
                            NonZeroU32::new(size.width).unwrap(),
                            NonZeroU32::new(size.height).unwrap(),
                        )
                        .expect("failed to resize surface");
                    redraw = true;
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                if let AppState::Active { cursor_pos, .. } = &mut self.state {
                    *cursor_pos = Some((position.x as f32, position.y as f32));
                }
                self.layout.handle_event(AppEvent::PointerMove {
                    x: position.x as f32,
                    y: position.y as f32,
                });
            }
            WindowEvent::MouseInput { state, button, .. } if button == MouseButton::Left => {
                let cursor_pos = match &self.state {
                    AppState::Active { cursor_pos, .. } => *cursor_pos,
                    _ => None,
                };

                let Some((x, y)) = cursor_pos else {
                    return;
                };

                match state {
                    ElementState::Pressed => {
                        self.layout.handle_event(AppEvent::PointerDown { x, y })
                    }
                    ElementState::Released => {
                        self.layout.handle_event(AppEvent::PointerUp { x, y });
                        self.layout.handle_event(AppEvent::PointerClick { x, y });
                    }
                }

                redraw = true;
            }
            WindowEvent::RedrawRequested => redraw = true,
            _ => {}
        }

        if redraw {
            if let AppState::Active { surface, .. } = &mut self.state {
                Self::render_and_present(&mut self.layout, surface);
            }
        }
    }
}

impl WinitWindowRenderer {
    fn new(event_loop: &EventLoop<()>, layout: AppLayout) -> Self {
        Self {
            context: Context::new(event_loop.owned_display_handle()).expect("failed context"),
            state: AppState::Init,
            layout,
            next_tick: Instant::now() + Duration::from_secs(1),
        }
    }

    fn render_and_present(
        layout: &mut AppLayout,
        surface: &mut Surface<OwnedDisplayHandle, Winnie>,
    ) {
        let handle = surface.window();
        let window_size = handle.inner_size();
        layout.render_layout(Size {
            width: length(window_size.width as f32),
            height: length(window_size.height as f32),
        });

        let mut buffer = surface.buffer_mut().expect("failed to map buffer");
        layout.draw(buffer.as_mut(), window_size.width, window_size.height);
        buffer.present().expect("failed to present buffer");
    }

    pub(crate) fn run(layout: AppLayout) {
        let event_loop = EventLoop::new().expect("failed to create event loop");
        event_loop.set_control_flow(ControlFlow::Wait);

        let mut app = Self::new(&event_loop, layout);
        event_loop.run_app(&mut app).expect("failed to run app");
    }
}
