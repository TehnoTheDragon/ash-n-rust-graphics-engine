use std::sync::Arc;

use ash::vk;

use crate::gpu::{device::Device, frame::Frame, surface::Surface};

#[derive(Debug, thiserror::Error)]
pub enum SwapchainError {
    #[error("{0}")]
    VulkanResult(#[from] vk::Result),
}

pub struct Swapchain {
    pub(crate) device: Arc<Device>,
    pub(crate) surface: Arc<Surface>,
    pub(crate) loader: Arc<ash::khr::swapchain::Device>,
    pub(crate) extent: vk::Extent2D,
    pub(crate) format: vk::Format,
    pub(crate) color_space: vk::ColorSpaceKHR,
    pub raw: Arc<vk::SwapchainKHR>,
    
    pub images: Vec<vk::Image>,
    pub image_views: Vec<vk::ImageView>,
    pub rendering_ready: Vec<vk::Semaphore>,
    pub frames: Box<[Frame]>,
    pub frame_semaphore: vk::Semaphore,
    pub max_in_flight_frames: u8,
}

impl Swapchain {
    pub fn new(
        device: Arc<Device>,
        surface: Arc<Surface>,
        size: (u32, u32),
        max_in_flight_frames: u8,
    ) -> Result<Self, SwapchainError> {
        let loader = Arc::new(ash::khr::swapchain::Device::new(&device.instance.raw, &device.logical_device));
        let (swapchain, extent, format, color_space) = Self::create_swapchain(
            &device,
            &surface,
            &loader,
            size,
            max_in_flight_frames,
        )?;
        let swapchain = Arc::new(swapchain);
        let (images, image_views, rendering_ready, frames) = Self::create_frames(
            &device,
            &swapchain,
            &loader,
            format,
            max_in_flight_frames,
        )?;

        let mut frame_semaphore_type = vk::SemaphoreTypeCreateInfo::default()
            .semaphore_type(vk::SemaphoreType::TIMELINE)
            .initial_value(max_in_flight_frames as u64)
            ;
        let frame_semaphore = unsafe { device.logical_device.create_semaphore(&vk::SemaphoreCreateInfo::default().push_next(&mut frame_semaphore_type), None)? };
        
        Ok(Self {
            device,
            surface,
            loader,
            extent,
            format,
            color_space,
            raw: swapchain,

            images,
            image_views,
            rendering_ready,
            frames: frames.into_boxed_slice(),
            frame_semaphore,
            max_in_flight_frames,
        })
    }

    pub fn recreate(
        &mut self,
        size: Option<(u32, u32)>,
        max_in_flight_frames: Option<u8>,
    ) -> Result<(), SwapchainError> {
        // Try change max_in_flight_frames
        self.max_in_flight_frames = max_in_flight_frames.unwrap_or(self.max_in_flight_frames);

        // Create new swapchain
        unsafe { self.loader.destroy_swapchain(*self.raw, None); }
        let (swapchain, extent, format, color_space) = Self::create_swapchain(
            &self.device,
            &self.surface,
            &self.loader,
            size.unwrap_or((self.extent.width, self.extent.height)),
            self.max_in_flight_frames
        )?;
        self.extent = extent;
        self.format = format;
        self.color_space = color_space;
        self.raw = Arc::new(swapchain);

        // Recreate frames
        unsafe {
            for frame in &self.frames {
                frame.destroy(&self.device);
            }
            for image_view in &self.image_views {
                self.device.logical_device.destroy_image_view(*image_view, None);
            }
            for semaphore in &self.rendering_ready {
                self.device.logical_device.destroy_semaphore(*semaphore, None);
            }
        }
        let (images, image_views, rendering_ready, frames) = Self::create_frames(
            &self.device,
            &self.raw,
            &self.loader,
            format,
            self.max_in_flight_frames
        )?;
        self.images = images;
        self.image_views = image_views;
        self.rendering_ready = rendering_ready;
        self.frames = frames.into_boxed_slice();

        // Recreate frame semaphore (apparently it is not required and will work without it, but I will leave it here anyway)
        // unsafe {
        //     self.device.logical_device.destroy_semaphore(self.frame_semaphore, None);
        // }
        // let mut frame_semaphore_type = vk::SemaphoreTypeCreateInfo::default()
        //     .semaphore_type(vk::SemaphoreType::TIMELINE)
        //     .initial_value(self.max_in_flight_frames as u64)
        //     ;
        // let frame_semaphore = unsafe { self.device.logical_device.create_semaphore(&vk::SemaphoreCreateInfo::default().push_next(&mut frame_semaphore_type), None)? };
        // self.frame_semaphore = frame_semaphore;

        Ok(())
    }

    fn create_frames(
        device: &Arc<Device>,
        swapchain: &Arc<vk::SwapchainKHR>,
        loader: &Arc<ash::khr::swapchain::Device>,
        format: vk::Format,
        max_in_flight_frames: u8
    ) -> Result<(Vec<vk::Image>, Vec<vk::ImageView>, Vec<vk::Semaphore>, Vec<Frame>), SwapchainError> {
        let images = unsafe { loader.get_swapchain_images(**swapchain)? };
        let image_views = images
            .iter()
            .map(|image| {
                let subresource = vk::ImageSubresourceRange::default()
                    .aspect_mask(vk::ImageAspectFlags::COLOR)
                    .base_mip_level(0)
                    .level_count(1)
                    .base_array_layer(0)
                    .layer_count(1)
                    ;
                let image_view_info = vk::ImageViewCreateInfo::default()
                    .image(*image)
                    .view_type(vk::ImageViewType::TYPE_2D)
                    .format(format)
                    .subresource_range(subresource)
                    ;
                let image_view = unsafe { device.logical_device.create_image_view(
                    &image_view_info,
                    None
                ).unwrap_or(vk::ImageView::null()) };
                image_view
            })
            .collect();
        let ready_semaphores = images
            .iter()
            .map(|_| {
                let ready = unsafe { device.logical_device.create_semaphore(
                    &vk::SemaphoreCreateInfo::default(),
                    None
                ).unwrap_or(vk::Semaphore::null()) };
                ready
            })
            .collect();
        let frames = (0..max_in_flight_frames)
            .into_iter()
            .map(|_| {
                Frame::new(device).expect("Could not create frame resources")
            })
            .collect();
        Ok((images, image_views, ready_semaphores, frames))
    }

    fn create_swapchain(
        device: &Arc<Device>,
        surface: &Arc<Surface>,
        loader: &Arc<ash::khr::swapchain::Device>,
        size: (u32, u32),
        max_in_flight_frames: u8
    ) -> Result<(vk::SwapchainKHR, vk::Extent2D, vk::Format, vk::ColorSpaceKHR), SwapchainError> {
        let surface_capabilities = unsafe { surface.loader.get_physical_device_surface_capabilities(*device.physical_device, *surface.raw)? };
        let surface_present_modes = unsafe { surface.loader.get_physical_device_surface_present_modes(*device.physical_device, *surface.raw)? };
        let surface_formats = unsafe { surface.loader.get_physical_device_surface_formats(*device.physical_device, *surface.raw)? };
        let present_mode = surface_present_modes
            .iter()
            .max_by_key(|mode| {
                match **mode {
                    vk::PresentModeKHR::MAILBOX => 100,
                    vk::PresentModeKHR::FIFO_RELAXED => 50,
                    vk::PresentModeKHR::FIFO => 40,
                    vk::PresentModeKHR::IMMEDIATE => 10,
                    _ => 0
                }
            })
            .cloned()
            .unwrap_or(vk::PresentModeKHR::FIFO);
        let queue_family_indices = [device.graphics_queue.family_and_queue_indices.0];

        let extent = vk::Extent2D::default()
            .width(size.0.clamp(surface_capabilities.min_image_extent.width, surface_capabilities.max_image_extent.width))
            .height(size.1.clamp(surface_capabilities.min_image_extent.height, surface_capabilities.max_image_extent.height));
        let format = surface_formats.first().unwrap().format;
        let color_space = surface_formats.first().unwrap().color_space;

        let create_info = vk::SwapchainCreateInfoKHR::default()
            .surface(*surface.raw)
            .min_image_count((max_in_flight_frames as u32).max(surface_capabilities.min_image_count).min(surface_capabilities.max_image_count))
            .image_format(format)
            .image_color_space(color_space)
            .image_extent(extent)
            .image_array_layers(1)
            .image_usage(vk::ImageUsageFlags::COLOR_ATTACHMENT)
            .image_sharing_mode(vk::SharingMode::EXCLUSIVE)
            .queue_family_indices(&queue_family_indices)
            .pre_transform(surface_capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            ;
        let swapchain = unsafe { loader.create_swapchain(&create_info, None)? };

        Ok((swapchain, extent, format, color_space))
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        unsafe {
            for image_view in &self.image_views {
                self.device.logical_device.destroy_image_view(*image_view, None);
            }
            for semaphore in &self.rendering_ready {
                self.device.logical_device.destroy_semaphore(*semaphore, None);
            }
            self.device.logical_device.destroy_semaphore(self.frame_semaphore, None);
            for frame in &self.frames {
                frame.destroy(&self.device);
            }
            self.loader.destroy_swapchain(*self.raw, None);
        }
    }
}