use std::sync::Arc;

use ash::vk::{self};

use crate::gpu::{instance::Instance, queue::{Queue, QueueError}, surface::Surface};

#[derive(Debug, thiserror::Error)]
pub enum DeviceError {
    #[error("{0}")]
    VulkanResult(#[from] vk::Result),
    #[error("{0}")]
    QueueError(#[from] QueueError),
    #[error("No suitable physical device found")]
    NoSuitablePhysicalDeviceFound,
}

pub struct Device {
    pub(crate) instance: Arc<Instance>,
    pub physical_device: Arc<vk::PhysicalDevice>,
    pub logical_device: Arc<ash::Device>,
    pub graphics_queue: Arc<Queue>,
    pub transfer_queue: Arc<Queue>,
}

impl Device {
    pub fn new(
        instance: Arc<Instance>,
        surface: Arc<Surface>,
    ) -> Result<Self, DeviceError> {
        let physical_device = Self::find_physical_device(&instance)?;
        let queue_family_properties = unsafe { instance.raw.get_physical_device_queue_family_properties(physical_device) };
        let (queues, (graphics_queue_family, transfer_queue_family)) = Self::declare_queues(&queue_family_properties, physical_device, surface)?;
        let extensions = vec![
             ash::khr::swapchain::NAME.as_ptr()
        ];
        let mut features11 = vk::PhysicalDeviceVulkan11Features::default()
            ;
        let mut features12 = vk::PhysicalDeviceVulkan12Features::default()
            .timeline_semaphore(true)
            ;
        let mut features13 = vk::PhysicalDeviceVulkan13Features::default()
            .synchronization2(true)
            .dynamic_rendering(true)
            ;
        let mut features = vk::PhysicalDeviceFeatures2::default()
            .push_next(&mut features11)
            .push_next(&mut features12)
            .push_next(&mut features13)
            ;
        let create_info = vk::DeviceCreateInfo::default()
            .enabled_extension_names(&extensions)
            .queue_create_infos(&queues)
            .push_next(&mut features)
            ;
        let logical_device = Arc::new(unsafe { instance.raw.create_device(physical_device, &create_info, None)? });
        let graphics_queue = Arc::new(Queue::new(logical_device.clone(), graphics_queue_family, 0)?);
        let transfer_queue = Arc::new(Queue::new(logical_device.clone(), transfer_queue_family, 0)?);
        Ok(Self {
            instance,
            physical_device: Arc::new(physical_device),
            logical_device,
            graphics_queue,
            transfer_queue,
        })
    }

    fn declare_queues<'a>(
        queue_family_properties: &Vec<vk::QueueFamilyProperties>,
        physical_device: vk::PhysicalDevice,
        surface: Arc<Surface>
    ) -> Result<([vk::DeviceQueueCreateInfo<'a>; 2], (u32, u32)), DeviceError> {
        let graphics_queue_index = Self::find_queue(
            &queue_family_properties,
            |index, p| p.queue_count > 0
                && p.queue_flags.contains(vk::QueueFlags::GRAPHICS)
                && unsafe { surface.loader.get_physical_device_surface_support(physical_device, *index as u32, *surface.raw).unwrap_or(false) }
        )?;

        let transfer_queue_index = Self::find_queue(
            &queue_family_properties,
            |_, p| p.queue_count > 0
                && p.queue_flags.contains(vk::QueueFlags::TRANSFER)
                && !p.queue_flags.contains(vk::QueueFlags::GRAPHICS)
        )?;

        let queue_indices = (graphics_queue_index, transfer_queue_index);
        let queues = [
            vk::DeviceQueueCreateInfo::default()
                .queue_family_index(graphics_queue_index)
                .queue_priorities(&[1.0]),
            vk::DeviceQueueCreateInfo::default()
                .queue_family_index(transfer_queue_index)
                .queue_priorities(&[1.0])
        ];

        Ok((queues, queue_indices))
    }

    fn find_queue<'a>(
        queue_family_properties: &Vec<vk::QueueFamilyProperties>,
        mut predicate: impl FnMut(&usize, &vk::QueueFamilyProperties) -> bool
    ) -> Result<u32, DeviceError> {
        if let Some((index, _)) = queue_family_properties
            .iter()
            .enumerate()
            .find(|(index, properties)| predicate(index, properties))
            {
                return Ok(index as u32)
            }
        Err(DeviceError::NoSuitablePhysicalDeviceFound)
    }

    fn find_physical_device(
        instance: &Arc<Instance>,
    ) -> Result<vk::PhysicalDevice, DeviceError> {
        let physical_devices = unsafe { instance.raw.enumerate_physical_devices()? };
        let physical_device = physical_devices
            .iter()
            .max_by_key(|physical_device| {
                let properties = unsafe { instance.raw.get_physical_device_properties(**physical_device) };

                match properties.device_type {
                    vk::PhysicalDeviceType::DISCRETE_GPU => 100,
                    vk::PhysicalDeviceType::INTEGRATED_GPU => 50,
                    vk::PhysicalDeviceType::VIRTUAL_GPU => 20,
                    vk::PhysicalDeviceType::OTHER => 10,
                    vk::PhysicalDeviceType::CPU => 0,
                    _ => 0
                }
            });
        match physical_device {
            Some(physical_device) => Ok(*physical_device),
            None => Err(DeviceError::NoSuitablePhysicalDeviceFound),
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        unsafe {
            self.logical_device.destroy_device(None);
        }
    }
}