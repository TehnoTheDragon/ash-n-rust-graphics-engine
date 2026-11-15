use std::sync::Arc;

use ash::vk;

use crate::gpu::{device::Device, shader, swapchain::Swapchain};

#[derive(Debug, thiserror::Error)]
pub enum GraphicsPipelineError {
    #[error("{0}")]
    VulkanResult(#[from] vk::Result),
    #[error("Failed to create graphics pipeline(s): {}", _0.1)]
    CreationFailed((Vec<vk::Pipeline>, vk::Result)),
}

pub struct GraphicsPipeline {
    pub(crate) device: Arc<Device>,
    pub(crate) layout: Arc<vk::PipelineLayout>,
    pub raw: Arc<vk::Pipeline>
}

impl GraphicsPipeline {
    pub fn new(
        device: Arc<Device>,
        swapchain: &Swapchain,
    ) -> Result<Self, GraphicsPipelineError> {
        // Layout
        let layout_info = vk::PipelineLayoutCreateInfo::default()
            .set_layouts(&[])
            .push_constant_ranges(&[])
            ;
        let layout = Arc::new(unsafe { device.logical_device.create_pipeline_layout(&layout_info, None)? });

        // Shaders
        let vertex_shader_code = shader::compile_shader(include_str!("../main.vs"), shaderc::ShaderKind::Vertex).unwrap();
        let vertex_shader_module_info = vk::ShaderModuleCreateInfo::default()
            .code(&vertex_shader_code)
            ;
        let vertex_shader_module = unsafe { device.logical_device.create_shader_module(&vertex_shader_module_info, None)? };

        let fragment_shader_code = shader::compile_shader(include_str!("../main.fs"), shaderc::ShaderKind::Fragment).unwrap();
        let fragment_shader_module_info = vk::ShaderModuleCreateInfo::default()
            .code(&fragment_shader_code)
            ;
        let fragment_shader_module = unsafe { device.logical_device.create_shader_module(&fragment_shader_module_info, None)? };

        // Shader Stages
        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::default()
                .name(c"main")
                .stage(vk::ShaderStageFlags::VERTEX)
                .module(vertex_shader_module)
                ,
            vk::PipelineShaderStageCreateInfo::default()
                .name(c"main")
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .module(fragment_shader_module)
                ,
        ];

        // Vertex Input
        let vertex_input = vk::PipelineVertexInputStateCreateInfo::default()
            ;
        
        // Input Assembly
        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::default()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            ;

        // Depth & Stencil
        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo::default()
            .depth_test_enable(false)
            .depth_write_enable(false)
            .stencil_test_enable(false)
            ;

        // Viewport
        let viewport = vk::PipelineViewportStateCreateInfo::default()
            .viewport_count(1)
            .scissor_count(1)
            ;

        // Raster
        let raster = vk::PipelineRasterizationStateCreateInfo::default()
            .polygon_mode(vk::PolygonMode::FILL)
            .cull_mode(vk::CullModeFlags::BACK)
            .front_face(vk::FrontFace::COUNTER_CLOCKWISE)
            .line_width(1.0)
            ;

        // Multisample
        let multisample = vk::PipelineMultisampleStateCreateInfo::default()
            .rasterization_samples(vk::SampleCountFlags::TYPE_1)
            ;

        // Color blending
        let color_blend_attachments = [
            vk::PipelineColorBlendAttachmentState::default()
                .blend_enable(false)
                .color_write_mask(vk::ColorComponentFlags::RGBA)
        ];
        let color_blend_state = vk::PipelineColorBlendStateCreateInfo::default()
            .attachments(&color_blend_attachments)
            ;
        
        // Dynamic State
        let dynamic_states = [
            vk::DynamicState::VIEWPORT,
            vk::DynamicState::SCISSOR,
        ];
        let dynamic_state = vk::PipelineDynamicStateCreateInfo::default()
            .dynamic_states(&dynamic_states)
            ;

        // Rendering
        let rendering_color_attachment_formats = [swapchain.format];
        let mut rendering = vk::PipelineRenderingCreateInfo::default()
            .color_attachment_formats(&rendering_color_attachment_formats)
            ;

        // Pipeline
        let graphics_pipeline_info = vk::GraphicsPipelineCreateInfo::default()
            .layout(*layout)
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input)
            .input_assembly_state(&input_assembly)
            .depth_stencil_state(&depth_stencil)
            .viewport_state(&viewport)
            .rasterization_state(&raster)
            .multisample_state(&multisample)
            .color_blend_state(&color_blend_state)
            .dynamic_state(&dynamic_state)
            .render_pass(vk::RenderPass::null())
            .push_next(&mut rendering)
            ;
        let graphics_pipeline = unsafe {
            device.logical_device.create_graphics_pipelines(
                vk::PipelineCache::null(),
                &[graphics_pipeline_info],
                None
            )
                .map_err(|e| GraphicsPipelineError::CreationFailed(e))?
                .first()
                .cloned()
                .unwrap()
        };

        // Clean up
        unsafe {
            device.logical_device.destroy_shader_module(vertex_shader_module, None);
            device.logical_device.destroy_shader_module(fragment_shader_module, None);
        }

        // Pipeline (return)
        Ok(Self {
            device,
            layout,
            raw: Arc::new(graphics_pipeline)
        })
    }
}

impl Drop for GraphicsPipeline {
    fn drop(&mut self) {
        unsafe {
            self.device.logical_device.destroy_pipeline(*self.raw, None);
            self.device.logical_device.destroy_pipeline_layout(*self.layout, None);
        }
    }
}