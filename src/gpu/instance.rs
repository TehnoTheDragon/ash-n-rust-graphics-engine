use std::sync::Arc;

use ash::vk;

unsafe extern "system" fn vulkan_debug_utils_callback(
    message_severity: vk::DebugUtilsMessageSeverityFlagsEXT,
    message_type: vk::DebugUtilsMessageTypeFlagsEXT,
    p_callback_data: *const vk::DebugUtilsMessengerCallbackDataEXT,
    _p_user_data: *mut std::ffi::c_void,
) -> vk::Bool32 {
    let message = unsafe { std::ffi::CStr::from_ptr((*p_callback_data).p_message) };
    let severity = format!("{:?}", message_severity).to_lowercase();
    let ty = format!("{:?}", message_type).to_lowercase();
    println!("[Debug][{}][{}] {:?}", severity, ty, message);
    vk::FALSE
}

#[derive(Debug, thiserror::Error)]
pub enum InstanceError {
    #[error("Failed to load vulkan library: {0}")]
    LoadingError(#[from] ash::LoadingError),
    #[error("{0}")]
    VulkanResult(#[from] vk::Result)
}

#[derive(Clone)]
pub struct Instance {
    pub(crate) entry: Arc<ash::Entry>,
    pub raw: Arc<ash::Instance>,
}

impl Instance {
    pub fn new(
        display_handle: raw_window_handle::DisplayHandle,
    ) -> Result<Self, InstanceError> {
        let entry = unsafe { ash::Entry::load()? };
        let application_info = vk::ApplicationInfo::default()
            .api_version(vk::API_VERSION_1_3)
            ;
        let mut surface_extensions = ash_window::enumerate_required_extensions(display_handle.as_raw())?.to_vec();
        let mut extensions = vec![
            ash::ext::debug_utils::NAME.as_ptr(),
        ];
        extensions.append(&mut surface_extensions);
        let layers = vec![
            c"VK_LAYER_KHRONOS_validation".as_ptr()
        ];
        let mut debug_utils_info = Self::get_debug_utils_create_info();
        let create_info = vk::InstanceCreateInfo::default()
            .application_info(&application_info)
            .enabled_extension_names(&extensions)
            .enabled_layer_names(&layers)
            .push_next(&mut debug_utils_info)
            ;
        let instance = unsafe { entry.create_instance(&create_info, None)? };
        Ok(Self {
            entry: Arc::new(entry),
            raw: Arc::new(instance),
        })
    }

    fn get_debug_utils_create_info<'a>() -> vk::DebugUtilsMessengerCreateInfoEXT<'a> {
        vk::DebugUtilsMessengerCreateInfoEXT {
            message_severity: ash::vk::DebugUtilsMessageSeverityFlagsEXT::WARNING
                | ash::vk::DebugUtilsMessageSeverityFlagsEXT::ERROR,
            message_type: ash::vk::DebugUtilsMessageTypeFlagsEXT::GENERAL
                | ash::vk::DebugUtilsMessageTypeFlagsEXT::PERFORMANCE
                | ash::vk::DebugUtilsMessageTypeFlagsEXT::VALIDATION,
            pfn_user_callback: Some(vulkan_debug_utils_callback),
            ..Default::default()
        }
    }
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { self.raw.destroy_instance(None); }
    }
}