use std::env;
use std::path::PathBuf;
// 本文件用于自动化生成C语言的头文件（header file）


fn main() {
    //打印出build.rs，这通常是Rust构建脚本的文件名。
    println!("build.rs");
    //获取环境变量CARGO_MANIFEST_DIR的值，这代表了Cargo的清单文件（Cargo.toml）所在的目录，然后打印出这个目录的路径。
    let crate_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")
        .expect("CARGO_MANIFEST_DIR env var is not defined"));
    println!("CARGO_MANIFEST_DIR: {:?}", crate_dir);
    //获取环境变量OUT_DIR的值，这是Cargo在编译过程中用于存放输出的临时文件的目录，然后打印出这个目录的路径。
    let out_dir = PathBuf::from(env::var("OUT_DIR")
        .expect("OUT_DIR env var is not defined"));
    println!("OUT_DIR: {:?}", out_dir);
    //读取cbindgen.toml配置文件，这个文件包含了cbindgen工具生成C头文件所需的配置信息。
    let config = cbindgen::Config::from_file("cbindgen.toml")
        .expect("Unable to find cbindgen.toml configuration file");

    // OUT_DIR doesn't appear to be configurable, so prolly not a good destination
    // cargo +nightly build --out-dir `pwd` -Z unstable-options
    // added question to this issue: https://github.com/rust-lang/cargo/issues/6790
    // for now, CARGO_MANIFEST_DIR (crate_dir) seems reasonable
      
    //使用cbindgen工具和上述配置，以及CARGO_MANIFEST_DIR所指向的目录作为输入，生成C头文件。
    cbindgen::generate_with_config(&crate_dir, config)
        .unwrap()
        .write_to_file(crate_dir.join("libnyx.h"));
    
}
