use std::sync::Arc;

use ash::vk;

#[derive(Debug, thiserror::Error)]
pub enum QueueError {
    #[error("{0}")]
    VulkanResult(#[from] vk::Result),
}

pub struct Queue {
    pub(crate) logical_device: Arc<ash::Device>,
    pub family_and_queue_indices: Arc<(u32, u32)>,
    pub raw: Arc<vk::Queue>,
}

impl Queue {
    pub fn new(
        logical_device: Arc<ash::Device>,
        queue_family_index: u32,
        queue_index: u32,
    ) -> Result<Self, QueueError> {
        let queue = Arc::new(unsafe { logical_device.get_device_queue(queue_family_index, queue_index) });
        Ok(Self {
            logical_device,
            family_and_queue_indices: Arc::new((queue_family_index, queue_index)),
            raw: queue,
        })
    }
}