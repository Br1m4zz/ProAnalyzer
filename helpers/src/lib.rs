extern crate nix;

use core::ffi::c_void;
use nix::sys::mman::*;
use std::fs::{File, OpenOptions};
use std::os::unix::io::IntoRawFd;

mod hash_by_ref;
pub use hash_by_ref::HashAsRef;
//根据指定的路径path的文件，创建该文件的mmap的共享内存
pub fn make_shared_data_from_path(path: &str, size: usize) -> &'static mut[u8] {
    let data_shm_f = OpenOptions::new()
    .create(true)
    .read(true)
    .write(true)
    .open(path)
    .expect("couldn't open input file");
    data_shm_f.set_len(size as u64).unwrap();
    return make_shared_data_from_file(data_shm_f, size);
}
//将给定的文件mmap映射到内存，然后使用 mmap 系统调用来创建一个内存映射
pub fn make_shared_data_from_file(file: File, size: usize) -> &'static mut [u8] {
    let prot = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
    let flags = MapFlags::MAP_SHARED;
    unsafe {
        let ptr = mmap(0 as *mut c_void, size, prot, flags, file.into_raw_fd(), 0).unwrap();

        let data = std::slice::from_raw_parts_mut(ptr as *mut u8, size);
        return data;
    }
}
