use std::sync::Arc;

use ash::vk;

use crate::gpu::device::Device;

pub struct Frame {
    pub(crate) command_pool: vk::CommandPool,
    pub(crate) command_buffer: vk::CommandBuffer,
    pub(crate) image_acquired: vk::Semaphore,
}

impl Frame {
    pub(crate) fn new(
        device: &Arc<Device>,
    ) -> Result<Self, vk::Result> {
        let command_pool_info = vk::CommandPoolCreateInfo::default()
            .queue_family_index(device.graphics_queue.family_and_queue_indices.0)
            ;
        let command_pool = unsafe { device.logical_device.create_command_pool(&command_pool_info, None)?};
        let command_buffer_allocate_info = vk::CommandBufferAllocateInfo::default()
            .command_pool(command_pool)
            .command_buffer_count(1)
            .level(vk::CommandBufferLevel::PRIMARY)
            ;
        let command_buffer = unsafe { device.logical_device.allocate_command_buffers(&command_buffer_allocate_info)?.first().cloned().unwrap() };
        let image_acquired = unsafe { device.logical_device.create_semaphore(&vk::SemaphoreCreateInfo::default(), None)? };
        Ok(Self {
            command_pool,
            command_buffer,
            image_acquired,
        })
    }

    pub(crate) fn destroy(&self, device: &Arc<Device>) {
        unsafe {
            device.logical_device.destroy_command_pool(self.command_pool, None);
            device.logical_device.destroy_semaphore(self.image_acquired, None);
        }
    }
}