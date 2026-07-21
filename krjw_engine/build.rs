#[cfg(windows)]
use crate::windows::d3d11_shaders;



#[cfg(windows)]
mod windows
{
    use anyhow::Result;
    use windows::Win32::Foundation::{ERROR_FILE_NOT_FOUND, ERROR_INVALID_NAME};
    use windows::Win32::Graphics::Direct3D::{*};
    use windows::{core::PCSTR};

    use std::ffi::{CStr, c_void};
    use std::path::{PathBuf};
    use std::sync::atomic::{AtomicU32};

    // 我们的自定义 Include 处理器
    struct MyInclude {
        _ref_count: AtomicU32,
        root_path: PathBuf,
        // 缓存已打开的文件内容，可选项
        // cache: HashMap<String, Vec<u8>>,
    }

    // 真的能用
    impl MyInclude {
        fn new(root_path: impl Into<PathBuf>) -> Self {
            Self {
                _ref_count: AtomicU32::new(1),
                root_path: root_path.into(),
            }
        }
    }

    // FFI 麻烦死了
    impl ID3DInclude_Impl for MyInclude {
        fn Open(
            &self,
            _includetype: D3D_INCLUDE_TYPE,
            pfilename: &PCSTR,
            _pparentdata: *const c_void,
            ppdata: *mut *mut c_void,
            pbytes: *mut u32,
        ) -> windows_core::Result<()> {
            let file_name = unsafe { CStr::from_ptr(pfilename.0 as *const _ as *const i8) };
            let file_name = file_name.to_str()
                .map_err(|_| windows_core::Error::from(ERROR_INVALID_NAME))?;

            let full_path = self.root_path.join(file_name);
            println!("DBG Open: filename {}", file_name);
            println!("DBG Open: full_path {:?}", full_path);

            let content = std::fs::read(&full_path)
                .map_err(|_| windows_core::Error::from(ERROR_FILE_NOT_FOUND))?;

            let content = String::from_utf8(content.clone()).unwrap();

            println!("DBG Open: readed, size = {}, content = \n{}[EOF]", content.len(), content);

            let len = content.len() as u32;
            // 将 Vec 放入 Box，然后转为裸指针
            let boxed_vec = content.into_boxed_str();
            let ptr = Box::into_raw(boxed_vec) as *mut c_void;

            unsafe {
                *ppdata = ptr;
                *pbytes = len;
            }

            println!("DBG Open: OK");
            Ok(()) // S_OK
        }

        fn Close(&self, pdata: *const c_void) -> windows_core::Result<()> {
            if pdata.is_null() {
                return Ok(());
            }

            println!("DBG Close: From raw start");
            // 将裸指针恢复为 Box<Vec<u8>>，自动释放
            unsafe {
                let _ = Box::from_raw(pdata as *mut u8);
            }
            Ok(())
        }
    }


    #[must_use]
    pub fn compile_shader(dir: impl Into<PathBuf>, source: &[u8], entrypoint: PCSTR, target: PCSTR) -> Result<Vec<u8>> {
        use windows::Win32::Graphics::Direct3D::Fxc::*;
        let mut shader_blob = None;
        let mut error_blob = None;

        let dir = dir.into();
        println!("DBG compile_shader: dir {:?}", dir);
        let inc = MyInclude::new(dir);
        let inc = ID3DInclude::new(&inc);

        let hr = unsafe {
            D3DCompile(
                source.as_ptr() as *const _,
                source.len(),
                PCSTR::null(),
                None,
                Some(&(inc.clone())), // D3D_COMPILE_STANDARD_FILE_INCLUDE 
                entrypoint,
                target,
                0,
                0,
                &mut shader_blob,
                Some(&mut error_blob),
            )
        };

        let blob = shader_blob.ok_or_else(|| {
            let msg = error_blob
                .as_ref()
                .map(|blob| unsafe {
                    String::from_utf8_lossy(std::slice::from_raw_parts(
                        blob.GetBufferPointer() as *const u8,
                        blob.GetBufferSize(),
                    ))
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


    pub fn d3d11_shaders() {
        let current_dir = std::env::current_dir().unwrap();
        let shaders_dir = current_dir.join("src").join("graphic").join("d3d11").join("shaders");

        for i in std::fs::read_dir(shaders_dir).unwrap() {
            let i = i.unwrap();
            let filetype = i.file_type().unwrap();
            if filetype.is_file() {
                let path = i.path();
                let pstr = path.to_str().unwrap();
                let fstem = path.file_stem().unwrap().to_os_string().into_string().expect("Unsupported path name");
                let dir = { let mut i = path.clone(); i.pop(); i };
                let target = if let Some(ext) = path.extension() {
                    let ext = ext.to_os_string().into_string().expect("Unsupported path name");
                    match ext.as_str() {
                        "vsh" => PCSTR("vs_5_0\0".as_ptr() as *const _),
                        "psh" => PCSTR("ps_5_0\0".as_ptr() as *const _),
                        _ => continue,
                    }
                } else { continue };
                println!("cargo:rerun-if-changed=\"{}\"", pstr);

                let out_path = dir.join([fstem, ".vs.cso".to_string()].join(""));

                let compiled = compile_shader(&dir, &std::fs::read(path).expect("Failed to read"), 
                    PCSTR("main\0".as_ptr() as *const _), target).expect("Compiling failed");
                
                std::fs::write(&out_path, compiled).expect(&format!("Writing to {} failed", out_path.to_str().unwrap()));
            }
        }
    }
}

fn main() {
    #[cfg(windows)]
    d3d11_shaders();
}
