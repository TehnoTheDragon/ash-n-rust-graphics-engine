use std::sync::Arc;

use ash::vk;

use crate::gpu::instance::Instance;

#[derive(Debug, thiserror::Error)]
pub enum SurfaceError {
    #[error("{0}")]
    VulkanResult(#[from] vk::Result)
}

pub struct Surface {
    pub(crate) instance: Arc<Instance>,
    pub(crate) loader: Arc<ash::khr::surface::Instance>,
    pub raw: Arc<vk::SurfaceKHR>,
}

impl Surface {
    pub fn new(
        instance: Arc<Instance>,
        display_handle: raw_window_handle::DisplayHandle,
        window_handle: raw_window_handle::WindowHandle,
    ) -> Result<Self, SurfaceError> {
        let loader = Arc::new(ash::khr::surface::Instance::new(&instance.entry, &instance.raw));
        let surface = unsafe { ash_window::create_surface(&instance.entry, &instance.raw, display_handle.as_raw(), window_handle.as_raw(), None)? };
        Ok(Self {
            instance,
            loader,
            raw: Arc::new(surface),
        })
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe { self.loader.destroy_surface(*self.raw, None); }
    }
}