pub fn compile_shader<'source, Source: Into<&'source str>>(
    source: Source,
    kind: shaderc::ShaderKind,
) -> Result<Vec<u32>, shaderc::Error> {
    let source = source.into();
    let compiler = shaderc::Compiler::new()?;
    let mut options = shaderc::CompileOptions::new()?;
    options.set_source_language(shaderc::SourceLanguage::GLSL);
    options.set_target_env(shaderc::TargetEnv::Vulkan, shaderc::EnvVersion::Vulkan1_3 as u32);
    options.set_target_spirv(shaderc::SpirvVersion::V1_6);
    options.set_invert_y(true);
    let spirv = compiler.compile_into_spirv(
        source,
        kind,
        "*.shader",
        "main",
        Some(&options)
    )?;
    Ok(spirv.as_binary().to_owned())
}