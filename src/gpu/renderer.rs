use std::sync::Arc;

use ash::vk;
use raw_window_handle::{HandleError, HasDisplayHandle, HasWindowHandle};
use winit::{window::Window};

use crate::gpu::{device::{Device, DeviceError}, graphics_pipeline::{self, GraphicsPipeline, GraphicsPipelineError}, instance::{Instance, InstanceError}, surface::{Surface, SurfaceError}, swapchain::{Swapchain, SwapchainError}};

#[derive(Debug, thiserror::Error)]
pub enum RendererError {
    #[error("{0}")]
    VulkanError(#[from] ash::vk::Result),
    #[error("{0}")]
    HandleError(#[from] HandleError),
    #[error("{0}")]
    InstanceError(#[from] InstanceError),
    #[error("{0}")]
    SurfaceError(#[from] SurfaceError),
    #[error("{0}")]
    DeviceError(#[from] DeviceError),
    #[error("{0}")]
    SwapchainError(#[from] SwapchainError),
    #[error("{0}")]
    GraphicsPipelineError(#[from] GraphicsPipelineError),
}

pub struct Renderer {
    window: Arc<Window>,
    instance: Arc<Instance>,
    surface: Arc<Surface>,
    device: Arc<Device>,
    swapchain: Swapchain,
    graphics_pipeline: GraphicsPipeline,
    
    swapchain_size_target: (u32, u32),
    require_swapchain_recreation: bool,
    frame_index: usize,
    next_signal_value: usize,
}

impl Renderer {
    pub fn new(
        window: Arc<Window>,
    ) -> Result<Self, RendererError> {
        let display_handle = window.display_handle()?;
        let window_handle = window.window_handle()?;
        let window_size: (u32, u32) = window.inner_size().into();
        
        let instance = Arc::new(Instance::new(display_handle)?);
        let surface = Arc::new(Surface::new(instance.clone(), display_handle, window_handle)?);
        let device = Arc::new(Device::new(instance.clone(), surface.clone())?);
        let swapchain = Swapchain::new(device.clone(), surface.clone(), window_size, 3)?;
        let graphics_pipeline = GraphicsPipeline::new(device.clone(), &swapchain)?;

        let next_signal_value = swapchain.max_in_flight_frames as usize;

        Ok(Self {
            window,
            instance,
            surface,
            device,
            swapchain,
            graphics_pipeline,

            swapchain_size_target: window_size,
            require_swapchain_recreation: false,
            frame_index: 0,
            next_signal_value,
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) -> Result<(), RendererError> {
        self.swapchain_size_target = (width, height);
        if width > 0 && height > 0 {
            self.require_swapchain_recreation = true;
        }
        Ok(())
    }

    pub fn destroy(&self) -> Result<(), RendererError> {
        Ok(unsafe { self.device.logical_device.device_wait_idle()? })
    }

    pub fn render(
        &mut self
    ) -> Result<(), RendererError> {
        // Skip rendering if target swapchain size is zero
        if self.swapchain_size_target.0 == 0 || self.swapchain_size_target.1 == 0 {
            return Ok(())
        }

        // Re-create swapchain if required
        if self.require_swapchain_recreation {
            unsafe { self.device.logical_device.device_wait_idle()? };
            self.swapchain.recreate(Some(self.swapchain_size_target), None)?;
            self.require_swapchain_recreation = false;
        }

        // Acquire values
        self.frame_index += 1;
        let frame_index = self.frame_index % self.swapchain.max_in_flight_frames as usize;
        self.next_signal_value += 1;
        let signal_value = self.next_signal_value;
        let wait_value = signal_value - self.swapchain.max_in_flight_frames as usize;

        // Wait for next frame to arrive
        {
            let wait_semaphores = [self.swapchain.frame_semaphore];
            let wait_values = [wait_value as u64];
            let wait_info = vk::SemaphoreWaitInfo::default()
                .semaphores(&wait_semaphores)
                .values(&wait_values)
                ;
            unsafe { self.device.logical_device.wait_semaphores(&wait_info, u64::MAX)? };
        }

        // Acquire next frame
        let frame = &mut self.swapchain.frames[frame_index];
        unsafe { self.device.logical_device.reset_command_pool(frame.command_pool, vk::CommandPoolResetFlags::empty())? };

        // Acquire next swapchain image
        let image_acquire = frame.image_acquired;
        let result = unsafe { self.swapchain.loader.acquire_next_image(
            *self.swapchain.raw,
            u64::MAX,
            image_acquire,
            vk::Fence::null()
        )};

        let (image_index, suboptimal) = match result {
            Ok(x) => x,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                self.require_swapchain_recreation = true;
                return Ok(());
            },
            Err(err) => panic!("{}", err),
        };

        if suboptimal {
            self.require_swapchain_recreation = true;
        }

        // Record command buffer of a current frame
        let command_buffer = frame.command_buffer;

        unsafe {
            // Begin command buffer
            self.device.logical_device.begin_command_buffer(
                command_buffer,
                &vk::CommandBufferBeginInfo::default()
                    .flags(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            )?;

            // Memory barrier
            {
                let memory_barriers = [
                    // Place a barrier to wait for swapchain image to be ready to be written
                    vk::ImageMemoryBarrier2::default()
                        .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                        .src_access_mask(vk::AccessFlags2::NONE)
                        .dst_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                        .dst_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                        .old_layout(vk::ImageLayout::UNDEFINED)
                        .new_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .image(self.swapchain.images[image_index as usize])
                        .subresource_range(vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(1))
                ];
                let dependency_info = vk::DependencyInfo::default()
                    .image_memory_barriers(&memory_barriers)
                    ;
                self.device.logical_device.cmd_pipeline_barrier2(command_buffer, &dependency_info);
            }

            // Rendering Attachments
            let color_attachments = [
                vk::RenderingAttachmentInfo::default()
                    .image_view(self.swapchain.image_views[image_index as usize])
                    .image_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                    .load_op(vk::AttachmentLoadOp::CLEAR)
                    .store_op(vk::AttachmentStoreOp::STORE)
                    .clear_value(vk::ClearValue {
                        color: vk::ClearColorValue {
                            float32: [0.0, 0.0, 0.067, 1.0]
                        }
                    })
            ];
            let rendering_info = vk::RenderingInfo::default()
                .render_area(vk::Rect2D::default()
                    .offset(vk::Offset2D::default())
                    .extent(self.swapchain.extent)
                )
                .layer_count(1)
                .color_attachments(&color_attachments)
                ;
            
            // Begin Rendering
            self.device.logical_device.cmd_begin_rendering(command_buffer, &rendering_info);
            {
                // Viewports
                let viewports = [
                    vk::Viewport::default()
                        .width(self.swapchain.extent.width as f32)
                        .height(self.swapchain.extent.height as f32)
                ];
                self.device.logical_device.cmd_set_viewport(command_buffer, 0, &viewports);

                // Scissors
                let scissors = [
                    vk::Rect2D::default()
                        .offset(vk::Offset2D::default())
                        .extent(self.swapchain.extent)
                ];
                self.device.logical_device.cmd_set_scissor(command_buffer, 0, &scissors);
                
                // Draw
                self.device.logical_device.cmd_bind_pipeline(command_buffer, vk::PipelineBindPoint::GRAPHICS, *self.graphics_pipeline.raw);
                self.device.logical_device.cmd_draw(command_buffer, 3, 1, 0, 0);
            }
            // End Rendering
            self.device.logical_device.cmd_end_rendering(command_buffer);

            // Memory barrier
            {
                let memory_barriers = [
                    // Place a barrier to wait for swapchain image to be ready to be present
                    vk::ImageMemoryBarrier2::default()
                        .src_stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
                        .src_access_mask(vk::AccessFlags2::COLOR_ATTACHMENT_WRITE)
                        .dst_stage_mask(vk::PipelineStageFlags2::NONE)
                        .dst_access_mask(vk::AccessFlags2::NONE)
                        .old_layout(vk::ImageLayout::COLOR_ATTACHMENT_OPTIMAL)
                        .new_layout(vk::ImageLayout::PRESENT_SRC_KHR)
                        .image(self.swapchain.images[image_index as usize])
                        .subresource_range(vk::ImageSubresourceRange::default()
                            .aspect_mask(vk::ImageAspectFlags::COLOR)
                            .base_mip_level(0)
                            .level_count(1)
                            .base_array_layer(0)
                            .layer_count(1))
                ];
                let dependency_info = vk::DependencyInfo::default()
                    .image_memory_barriers(&memory_barriers)
                    ;
                self.device.logical_device.cmd_pipeline_barrier2(command_buffer, &dependency_info);
            }

            // End command buffer
            self.device.logical_device.end_command_buffer(command_buffer)?;
        }

        // Ensure current swapchain image is available to start color output
        {
            let image_acquire_wait_infos = [
                vk::SemaphoreSubmitInfo::default()
                    .semaphore(image_acquire)
                    .stage_mask(vk::PipelineStageFlags2::COLOR_ATTACHMENT_OUTPUT)
            ];
            let semaphore_signals = [
                vk::SemaphoreSubmitInfo::default()
                    .semaphore(self.swapchain.rendering_ready[image_index as usize])
                    .stage_mask(vk::PipelineStageFlags2::ALL_GRAPHICS)
                    ,
                vk::SemaphoreSubmitInfo::default()
                    .semaphore(self.swapchain.frame_semaphore)
                    .value(signal_value as u64)
                    .stage_mask(vk::PipelineStageFlags2::ALL_COMMANDS)
                    ,
            ];
            let command_buffer_submit_infos = [
                vk::CommandBufferSubmitInfo::default()
                    .command_buffer(command_buffer)
            ];
            let submit_infos = [
                vk::SubmitInfo2::default()
                    .wait_semaphore_infos(&image_acquire_wait_infos)
                    .command_buffer_infos(&command_buffer_submit_infos)
                    .signal_semaphore_infos(&semaphore_signals)
            ];
            
            unsafe { self.device.logical_device.queue_submit2(*self.device.graphics_queue.raw, &submit_infos, vk::Fence::null())? };
        }

        // Present
        {
            let wait_semaphores = [
                self.swapchain.rendering_ready[image_index as usize]
            ];
            let swapchains = [
                *self.swapchain.raw
            ];
            let image_indices = [
                image_index
            ];
            let present_info = vk::PresentInfoKHR::default()
                .wait_semaphores(&wait_semaphores)
                .swapchains(&swapchains)
                .image_indices(&image_indices)
                ;
            unsafe { self.swapchain.loader.queue_present(*self.device.graphics_queue.raw, &present_info)? };
        }

        Ok(())
    }
}