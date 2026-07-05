use anyhow::Result;
use windows::{
    Win32::Graphics::{Direct3D::Fxc::*},
    core::PCSTR,
};

pub fn compile_shader(source: &[u8], entrypoint: PCSTR, target: PCSTR) -> Result<Vec<u8>> {
    let mut shader_blob = None;
    let mut error_blob = None;

    let hr = unsafe {
        D3DCompile(
            source.as_ptr() as *const _,
            source.len(),
            PCSTR::null(),
            None,
            None,
            entrypoint,
            target,
            0,
            0,
            &mut shader_blob,
            Some(&mut error_blob),
        )
    };

    let blob = shader_blob.ok_or_else(|| {
        let msg = error_blob.as_ref().map(|blob| unsafe {
            String::from_utf8_lossy(
                std::slice::from_raw_parts(
                    blob.GetBufferPointer() as *const u8,
                    blob.GetBufferSize(),
                ),
            )
            .into_owned()
        })
        .unwrap_or_else(|| format!("D3DCompile returned {:?}", hr));
        anyhow::anyhow!("D3DCompile failed\n{}", msg)
    })?;

    Ok(unsafe {
        std::slice::from_raw_parts(blob.GetBufferPointer() as *const u8, blob.GetBufferSize())
            .to_vec()
    })
}