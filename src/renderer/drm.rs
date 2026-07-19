use std::{fs::OpenOptions, path::Path, thread, time::Duration};

use drm::{
    Device,
    buffer::{Buffer as _, DrmFourcc},
    control::{self, Device as CtrlDevice, connector, crtc},
};
use taffy::{Size, prelude::length};

use crate::AppLayout;

pub(crate) struct DrmWindowRenderer;

impl DrmWindowRenderer {
    pub(crate) fn run(mut layout: AppLayout) {
        Self::try_run(&mut layout).expect("failed to run DRM window renderer");
    }

    fn try_run(layout: &mut AppLayout) -> Result<(), Box<dyn std::error::Error>> {
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

        println!(
            "DRM target: connector={} encoder={:?} crtc={:?} mode={}x{}",
            connector, encoder, crtc, width, height
        );

        layout.render_layout(Size {
            width: length(width as f32),
            height: length(height as f32),
        });

        let mut frame = vec![0_u32; (width * height) as usize];
        layout.draw(&mut frame, width, height);

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

        loop {
            thread::sleep(Duration::from_secs(60));
        }
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

impl Device for Card {}
impl CtrlDevice for Card {}
