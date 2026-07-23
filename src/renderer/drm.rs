use std::{
    fs::{OpenOptions, read_dir},
    io::ErrorKind,
    path::Path,
    thread,
    time::{Duration, Instant},
};

use drm::{
    Device as DrmDevice,
    buffer::{Buffer as _, DrmFourcc},
    control::{self, Device as CtrlDevice, connector, crtc},
};
use evdev::{AbsoluteAxisCode, Device as EvdevDevice, EventSummary, KeyCode};
use taffy::{Size, prelude::length};
use tracing::info;

use crate::app::App;
use crate::event::AppEvent;

pub(crate) struct DrmWindowRenderer;

impl DrmWindowRenderer {
    pub(crate) fn run(mut app: App) {
        Self::try_run(&mut app).expect("failed to run DRM window renderer");
    }

    fn try_run(app: &mut App) -> Result<(), Box<dyn std::error::Error>> {
        let device = Card::find()?;
        let resources = device.resource_handles()?;

        let connector = resources
            .connectors()
            .iter()
            .find_map(|&handle| {
                let connector = device.get_connector(handle, true).ok()?;
                (connector.state() == connector::State::Connected).then_some(connector)
            })
            .ok_or("no connected DRM connector found")?;

        let mode = connector
            .modes()
            .iter()
            .find(|mode| mode.mode_type().contains(control::ModeTypeFlags::PREFERRED))
            .copied()
            .or_else(|| connector.modes().first().copied())
            .ok_or("no DRM mode found")?;

        let (encoder, crtc) = Self::select_encoder_and_crtc(&device, &resources, &connector)?;
        let (width, height) = mode.size();
        let (width, height) = (width as u32, height as u32);

        info!(?connector, ?encoder, ?crtc, width, height, "DRM target selected");

        app.render_layout(Size {
            width: length(width as f32),
            height: length(height as f32),
        });

        let mut frame = vec![0_u32; (width * height) as usize];
        app.draw(&mut frame, width, height);

        let mut dumb = device.create_dumb_buffer((width, height), DrmFourcc::Xrgb8888, 32)?;
        let framebuffer = device.add_framebuffer(&dumb, 24, 32)?;
        let pitch = dumb.pitch() as usize;
        let mut mapping = device.map_dumb_buffer(&mut dumb)?;

        Self::copy_frame_to_dumb_buffer(
            &frame,
            &mut mapping,
            width as usize,
            height as usize,
            pitch,
        );

        device.set_crtc(
            crtc,
            Some(framebuffer),
            (0, 0),
            &[connector.handle()],
            Some(mode),
        )?;

        let mut touch_devices = Self::open_touch_devices()?;
        let mut next_tick = Instant::now() + Duration::from_secs(1);

        loop {
            let mut dirty = app.poll_sync();

            for touch in &mut touch_devices {
                dirty |= touch.poll(app, width, height);
            }

            let now = Instant::now();
            if now >= next_tick {
                app.handle_event(AppEvent::Tick);
                dirty = true;
                while now >= next_tick {
                    next_tick += Duration::from_secs(1);
                }
            }

            if dirty {
                app.render_layout(Size {
                    width: length(width as f32),
                    height: length(height as f32),
                });

                let mut frame = vec![0_u32; (width * height) as usize];
                app.draw(&mut frame, width, height);
                Self::copy_frame_to_dumb_buffer(
                    &frame,
                    &mut mapping,
                    width as usize,
                    height as usize,
                    pitch,
                );
            }

            thread::sleep(Duration::from_millis(16));
        }
    }

    fn open_touch_devices() -> Result<Vec<TouchDevice>, Box<dyn std::error::Error>> {
        let mut devices = Vec::new();

        for entry in read_dir("/dev/input")? {
            let entry = entry?;
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };

            if !name.starts_with("event") {
                continue;
            }

            if let Some(device) = TouchDevice::open(&path) {
                devices.push(device);
            }
        }

        Ok(devices)
    }

    fn select_encoder_and_crtc(
        device: &Card,
        resources: &control::ResourceHandles,
        connector: &connector::Info,
    ) -> Result<(control::encoder::Handle, crtc::Handle), Box<dyn std::error::Error>> {
        let preferred = connector.current_encoder().into_iter();
        let available = connector.encoders().iter().copied();

        for encoder_handle in preferred.chain(available) {
            let encoder = match device.get_encoder(encoder_handle) {
                Ok(encoder) => encoder,
                Err(_) => continue,
            };

            let possible_crtcs = resources.filter_crtcs(encoder.possible_crtcs());
            if let Some(crtc) = encoder
                .crtc()
                .filter(|crtc| possible_crtcs.contains(crtc))
                .or_else(|| possible_crtcs.first().copied())
            {
                return Ok((encoder.handle(), crtc));
            }
        }

        Err("no compatible DRM encoder/CRTC found".into())
    }

    fn copy_frame_to_dumb_buffer(
        frame: &[u32],
        mapping: &mut [u8],
        width: usize,
        height: usize,
        pitch: usize,
    ) {
        let row_bytes = width * std::mem::size_of::<u32>();

        for y in 0..height {
            let src_row = &frame[y * width..(y + 1) * width];
            let dst_row = &mut mapping[y * pitch..y * pitch + row_bytes];

            for (pixel, dst) in src_row.iter().zip(dst_row.chunks_exact_mut(4)) {
                dst.copy_from_slice(&pixel.to_ne_bytes());
            }
        }
    }
}

struct TouchDevice {
    device: EvdevDevice,
    x_range: Option<AxisRange>,
    y_range: Option<AxisRange>,
    raw_x: Option<i32>,
    raw_y: Option<i32>,
    pressed: bool,
}

#[derive(Clone, Copy)]
struct AxisRange {
    min: i32,
    max: i32,
}

impl TouchDevice {
    fn open(path: impl AsRef<Path>) -> Option<Self> {
        let mut device = EvdevDevice::open(path).ok()?;

        if !Self::is_touchscreen(&device) {
            return None;
        }

        device.set_nonblocking(true).ok()?;

        let x_range = Self::axis_range(&device, AbsoluteAxisCode::ABS_X)
            .or_else(|| Self::axis_range(&device, AbsoluteAxisCode::ABS_MT_POSITION_X));
        let y_range = Self::axis_range(&device, AbsoluteAxisCode::ABS_Y)
            .or_else(|| Self::axis_range(&device, AbsoluteAxisCode::ABS_MT_POSITION_Y));

        Some(Self {
            device,
            x_range,
            y_range,
            raw_x: None,
            raw_y: None,
            pressed: false,
        })
    }

    fn is_touchscreen(device: &EvdevDevice) -> bool {
        let has_abs = device.supported_absolute_axes().is_some_and(|axes| {
            axes.contains(AbsoluteAxisCode::ABS_X)
                || axes.contains(AbsoluteAxisCode::ABS_Y)
                || axes.contains(AbsoluteAxisCode::ABS_MT_POSITION_X)
                || axes.contains(AbsoluteAxisCode::ABS_MT_POSITION_Y)
        });

        let has_touch = device
            .supported_keys()
            .is_some_and(|keys| keys.contains(KeyCode::BTN_TOUCH));

        has_abs && has_touch
    }

    fn axis_range(device: &EvdevDevice, axis: AbsoluteAxisCode) -> Option<AxisRange> {
        device.get_absinfo().ok()?.find_map(|(code, info)| {
            (code == axis).then_some(AxisRange {
                min: info.minimum(),
                max: info.maximum(),
            })
        })
    }

    fn poll(&mut self, app: &mut App, width: u32, height: u32) -> bool {
        let mut dirty = false;

        loop {
            let events: Vec<_> = match self.device.fetch_events() {
                Ok(events) => events.collect(),
                Err(err) if err.kind() == ErrorKind::WouldBlock => break,
                Err(_) => break,
            };

            for event in events {
                dirty |= self.handle_event(app, event, width, height);
            }
        }

        dirty
    }

    fn handle_event(
        &mut self,
        app: &mut App,
        event: evdev::InputEvent,
        width: u32,
        height: u32,
    ) -> bool {
        match event.destructure() {
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_X, value)
            | EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_POSITION_X, value) => {
                self.raw_x = Some(value);
                if self.pressed {
                    if let Some((x, y)) = self.position(width, height) {
                        app.handle_event(AppEvent::PointerMove { x, y });
                    }
                }
                true
            }
            EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_Y, value)
            | EventSummary::AbsoluteAxis(_, AbsoluteAxisCode::ABS_MT_POSITION_Y, value) => {
                self.raw_y = Some(value);
                if self.pressed {
                    if let Some((x, y)) = self.position(width, height) {
                        app.handle_event(AppEvent::PointerMove { x, y });
                    }
                }
                true
            }
            EventSummary::Key(_, KeyCode::BTN_TOUCH, value) if value > 0 => {
                self.pressed = true;
                if let Some((x, y)) = self.position(width, height) {
                    app.handle_event(AppEvent::PointerDown { x, y });
                }
                true
            }
            EventSummary::Key(_, KeyCode::BTN_TOUCH, value) if value == 0 => {
                self.pressed = false;
                if let Some((x, y)) = self.position(width, height) {
                    app.handle_event(AppEvent::PointerUp { x, y });
                    app.handle_event(AppEvent::PointerClick { x, y });
                }
                true
            }
            _ => false,
        }
    }

    fn position(&self, width: u32, height: u32) -> Option<(f32, f32)> {
        let x = self.scale(self.raw_x?, self.x_range?, width);
        let y = self.scale(self.raw_y?, self.y_range?, height);
        Some((x, y))
    }

    fn scale(&self, raw: i32, range: AxisRange, size: u32) -> f32 {
        let span = (range.max - range.min).max(1) as f32;
        let normalized = (raw - range.min).max(0) as f32 / span;
        normalized.clamp(0.0, 1.0) * size as f32
    }
}

struct Card(std::fs::File);

impl Card {
    fn find() -> Result<Self, Box<dyn std::error::Error>> {
        for i in 0..10 {
            let path = format!("/dev/dri/card{i}");
            let Ok(device) = Self::open(path) else {
                continue;
            };

            let Ok(handles) = device.resource_handles() else {
                continue;
            };

            if handles
                .connectors()
                .iter()
                .filter_map(|connector| device.get_connector(*connector, false).ok())
                .any(|connector| connector.state() == connector::State::Connected)
            {
                return Ok(device);
            }
        }

        Err("no DRM device found".into())
    }

    fn open(path: impl AsRef<Path>) -> Result<Self, Box<dyn std::error::Error>> {
        let file = OpenOptions::new().read(true).write(true).open(path)?;
        Ok(Self(file))
    }
}

impl std::os::fd::AsFd for Card {
    fn as_fd(&self) -> std::os::fd::BorrowedFd<'_> {
        self.0.as_fd()
    }
}

impl DrmDevice for Card {}
impl CtrlDevice for Card {}
