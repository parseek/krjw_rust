
#[cfg(windows)]
fn d3d11_shaders() {
    let current_dir = std::env::current_dir().unwrap();
    let shaders_dir = current_dir.join("src").join("graphic").join("d3d11").join("shaders");

    for i in std::fs::read_dir(shaders_dir).unwrap() {
        let i = i.unwrap();
        let filetype = i.file_type().unwrap();
        if filetype.is_file() {
            let filename = i.file_name().into_string().unwrap();
            let ext = filename.ends_with(".hlsl");
            if ext {
                println!("cargo::rerun-if-changed={}", i.path().to_str().unwrap());
            }
        }
    }
}

fn main() {
    if cfg!(windows) {
        d3d11_shaders();
    }
}