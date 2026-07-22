#[cfg(all(
    target_os = "linux",
    target_arch = "arm",
    target_abi = "eabihf",
    any(target_env = "gnu", target_env = "musl")
))]
pub(crate) mod drm;
#[cfg(not(all(
    target_os = "linux",
    target_arch = "arm",
    target_abi = "eabihf",
    any(target_env = "gnu", target_env = "musl")
)))]
pub(crate) mod winit;

use crate::app::App;

#[cfg(all(
    target_os = "linux",
    target_arch = "arm",
    target_abi = "eabihf",
    any(target_env = "gnu", target_env = "musl")
))]
pub(crate) fn run(app: App) {
    println!("Using DRM window renderer");
    drm::DrmWindowRenderer::run(app);
}

#[cfg(not(all(
    target_os = "linux",
    target_arch = "arm",
    target_abi = "eabihf",
    any(target_env = "gnu", target_env = "musl")
)))]
pub(crate) fn run(app: App) {
    println!("Using winit window renderer");
    winit::WinitWindowRenderer::run(app);
}
