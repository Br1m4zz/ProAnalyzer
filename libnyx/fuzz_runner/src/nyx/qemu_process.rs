use core::ffi::c_void;
use nix::sys::mman::*;
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::prelude::*;
use std::os::unix::fs::symlink;
use std::os::unix::io::IntoRawFd;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::{thread, time};

use std::str;

extern crate colored; // not needed in Rust 2018

use colored::*;


use crate::nyx::aux_buffer::AuxBuffer;
use crate::nyx::ijon_data::{SharedFeedbackData, FeedbackBuffer};
use crate::nyx::mem_barrier::mem_barrier;
use crate::nyx::params::QemuParams;

//qemu实例
pub struct QemuProcess {
    pub process: Child,
    pub aux: AuxBuffer,
    pub feedback_data: FeedbackBuffer,
    pub ctrl: UnixStream,
    pub bitmap: &'static mut [u8],
    pub payload: &'static mut [u8],
    pub params: QemuParams,
    hprintf_log: File,
}

/// 通过ctrl管道写入120，命令qemu执行
fn execute_qemu(ctrl: &mut UnixStream) {
    ctrl.write_all(&[120_u8]).unwrap();
}

/// 通过ctrl管道读取内容到buf
fn wait_qemu(ctrl: &mut UnixStream) {
    let mut buf = [0];
    ctrl.read_exact(&mut buf).unwrap();
}

///通过向ctrl的Unix stream命令qemu开始执行，并等待读取qemu的来自ctrl的任何响应
fn run_qemu(ctrl: &mut UnixStream) {
    execute_qemu(ctrl);
    wait_qemu(ctrl);
}

///创建共享内存，用于虚拟机内外交换信息。bitmap和payload的共享内存如此创建
fn make_shared_data(file: File, size: usize) -> &'static mut [u8] {
    let prot = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
    let flags = MapFlags::MAP_SHARED;
    unsafe {
        let ptr = mmap(0 as *mut c_void, size, prot, flags, file.into_raw_fd(), 0).unwrap();

        let data = std::slice::from_raw_parts_mut(ptr as *mut u8, size);
        return data;
    }
}

//创建ijon的共享信息
fn make_shared_ijon_data(file: File, size: usize) -> FeedbackBuffer {
    let prot = ProtFlags::PROT_READ | ProtFlags::PROT_WRITE;
    let flags = MapFlags::MAP_SHARED;
    unsafe {
        let ptr = mmap(std::ptr::null_mut::<c_void>(), 0x1000, prot, flags, file.into_raw_fd(), size as i64).unwrap();
        FeedbackBuffer::new((ptr as *mut SharedFeedbackData).as_mut().unwrap())
    }
}

//qemuProcess的运行实例
impl QemuProcess {

    ///根据params参数创建、执行qemu实例，直到auxbuffer提示可以用于fuzz了。
    ///
    /// 最终返回目标qemu的运行的子进程pid、通信管道、执行信息的文件句柄
    pub fn new(params: QemuParams) -> QemuProcess {
        Self::prepare_redqueen_workdir(&params.workdir, params.qemu_id);

        if params.qemu_id == 0{
            println!("[!] libnyx: spawning qemu with:\n {}", params.cmd.join(" "));
        }

    //创建两个共享内存文件，用于传递bitmap信息和payload
        let bitmap_shm_f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&params.bitmap_filename)
            .expect("couldn't open bitmap file");
        let mut payload_shm_f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&params.payload_filename)
            .expect("couldn't open payload file");

        symlink(
            &params.bitmap_filename,
            format!("{}/bitmap_{}", params.workdir, params.qemu_id),
        )
        .unwrap();
        symlink(
            &params.payload_filename,
            format!("{}/payload_{}", params.workdir, params.qemu_id),
        )
        .unwrap();
        //println!("======================================SET NOT_INIT!!!!");
        payload_shm_f.write_all(b"not_init").unwrap();
        bitmap_shm_f.set_len(params.bitmap_size as u64).unwrap();
        payload_shm_f.set_len(params.payload_size as u64 + 0x1000).unwrap();

        let bitmap_shared = make_shared_data(bitmap_shm_f, params.bitmap_size);
        let payload_shared = make_shared_data(payload_shm_f, params.payload_size);

        
        let bitmap_shm_f = OpenOptions::new()
            .create(true)
            .read(true)
            .write(true)
            .open(&params.bitmap_filename)
            .expect("couldn't open bitmap file");
        
        //与ijon特性相关
        let ijon_shared = make_shared_ijon_data(bitmap_shm_f, params.bitmap_size);

        
        thread::sleep(time::Duration::from_secs(1));

        thread::sleep(time::Duration::from_millis(200*params.qemu_id as u64));


        //根据是否需要使用Python脚本dump输出，配置环境变量（与复现相关，fuzz的时候不跑第一个）
        //创建并执行qemu 命令
        let child = if params.dump_python_code_for_inputs{
            Command::new(&params.cmd[0])
            .args(&params.cmd[1..])
            .env("DUMP_PAYLOAD_MODE", "TRUE")
            .spawn()
            .expect("failed to execute process")            
        }
        else{
            Command::new(&params.cmd[0])
            .args(&params.cmd[1..])
            .spawn()
            .expect("failed to execute process")
        };

        
        thread::sleep(time::Duration::from_secs(1));

        thread::sleep(time::Duration::from_millis(200*params.qemu_id as u64));

        //println!("CONNECT TO {}", params.control_filename);

        //control.settimeout(None) maybe needed?
        //control.setblocking(1)

        //循环尝试直到获取qemu运行后输出的的unixstream
        let mut control = loop {
            match UnixStream::connect(&params.control_filename) {
                Ok(stream) => break stream,
                _ => {
                    //println!("TRY..."); /* broken af */
                    thread::sleep(time::Duration::from_millis(1))
                },
            }
        };

        // dry_run
        //println!("TRHEAD {} run QEMU initial",params.qemu_id);
        //这里通过control 管道控制qemu的行为
        run_qemu(&mut control);

        //新建辅助缓冲区的共享文件
        let aux_shm_f = OpenOptions::new()
            .read(true)
            .write(true)
            .open(&params.qemu_aux_buffer_filename)
            .expect("couldn't open aux buffer file");
        aux_shm_f.set_len(0x1000).unwrap();

        let aux_shm_f = OpenOptions::new()
            .write(true)
            .read(true)
            .open(&params.qemu_aux_buffer_filename)
            .expect("couldn't open aux buffer file");
        //根据共享文件，创建aux_buffer
        let mut aux_buffer = AuxBuffer::new(aux_shm_f);

        aux_buffer.validate_header();
        aux_buffer.config.protect_payload_buffer = 1;

        //循环检测
        loop {
            //如果aux_buffer提示有触发hprintf,打印aux_buffer的misc中的数据
            if aux_buffer.result.hprintf == 1 {
                let len = aux_buffer.misc.len;
                print!("{}", String::from_utf8_lossy(&aux_buffer.misc.data[0..len as usize]).yellow());
            }
            else{
                //println!("QEMU NOT READY");
            }

            //3表示可以准备好fuzz了
            if aux_buffer.result.state == 3 {
                break;
            }
            //println!("QEMU NOT READY");
            //println!("TRHEAD {} run QEMU NOT READY",params.qemu_id);
            //执行run_qemu让qemu继续向后执行
            run_qemu(&mut control);
        }
        //println!("QEMU READY");
        println!("[!] libnyx: qemu #{} is ready:", params.qemu_id);
        //配置运行实例的运行限制
        aux_buffer.config.reload_mode = 1;
        aux_buffer.config.timeout_sec = 0;
        aux_buffer.config.timeout_usec = 500_000;
        aux_buffer.config.changed = 1;

        //run_qemu(&mut control);
        //run_qemu(&mut control);

        let mut option = OpenOptions::new();
        option.read(true);
        option.write(true);
        option.create(true);
        let hprintf_log = option.open(format!("{}/hprintf_log_{}", params.workdir, params.qemu_id)).unwrap(); 
        //返回目标准备好的qemu运行实例
        return QemuProcess {
            process: child,
            aux: aux_buffer,
            feedback_data: ijon_shared,
            ctrl: control,
            bitmap: bitmap_shared,
            payload: payload_shared,
            params,
            hprintf_log,
        };
    }

///发送payload：按照当前传入的spec、auxbuffer信息进行测试。收集运行信息
    pub fn send_payload(&mut self) {
        let mut old_address: u64 = 0;
        //use rand::Rng;
        //println!("RUN INPUT");
        //std::thread::sleep(std::time::Duration::from_secs(1));
        //let time = std::time::SystemTime::now().duration_since(std::time::SystemTime::UNIX_EPOCH).unwrap().as_nanos();
        //self.hprintf_log.write_all(&format!("===({})===\n", time).as_bytes()).unwrap();
        // 循环中观察aux_buffer中result的任何事件，某些事件产生均会中断循环

        loop {
            mem_barrier();
            run_qemu(&mut self.ctrl);
            mem_barrier();

            //hprintf有任何置位，将辅助缓冲区的misc部分的数据写入hprintf_log
            if self.aux.result.hprintf != 0 {
                self.hprintf_log.write_all(&format!("{}\n", self.aux.misc.as_string()).as_bytes()).unwrap();
                //println!("HPRINTF {}", self.aux.misc.as_string());
                let len = self.aux.misc.len;

                

                print!("{}", String::from_utf8_lossy(&self.aux.misc.data[0..len as usize]).yellow());
                //print!("{}", "".clear());
                println!("TEST\n");

                continue;
            }
            //println!("pt trace size {:x} bytes",self.aux.result.pt_trace_size);
            //println!("{:} dirty pages",self.aux.result.dirty_pages);
            //println!("interpreter ran {} ops",self.feedback_data.shared.interpreter.executed_opcode_num);
            //let max_v = 0;
            //let max_i = 0;
            //for (i,v) in self.feedback_data.shared.ijon.max_data.iter().enumerate(){
            //    if *v > max_v{
            //        max_v=*v;
            //        max_i=i;
            //        
            //    }
            //}
            //println!("found IJON MAX: {}\t{:x}",max_i,max_v);

            if self.aux.result.success != 0 || self.aux.result.crash_found != 0 || self.aux.result.asan_found != 0 || self.aux.result.payload_write_attempt_found != 0 {
                break;
            }

            //处理page_not_found
            if self.aux.result.page_not_found != 0 {
                let v = self.aux.result.page_not_found_addr;
                println!("PAGE NOT FOUND -> {:x}\n", v);
                if old_address == self.aux.result.page_not_found_addr {
                    break;
                }
                old_address = self.aux.result.page_not_found_addr;
                self.aux.config.page_addr = self.aux.result.page_not_found_addr;
                self.aux.config.page_dump_mode = 1;
                self.aux.config.changed = 1;
            } 
            //else {
            //    break;
            //}

        }
        //std::thread::sleep(std::time::Duration::from_secs(1));
        //if self.aux.result.tmp_snapshot_created != 0 {
        //    //println!("created snapshot!!!!!!\n");
        //}
    }

    ///通过aux_buffer设置超时时间
    pub fn set_timeout(&mut self, timeout: std::time::Duration){
        self.aux.config.timeout_sec = timeout.as_secs() as u8;
        self.aux.config.timeout_usec = timeout.subsec_micros();
        self.aux.config.changed = 1;
    }

    pub fn wait(&mut self) {
        self.process.wait().unwrap();
    }

    pub fn shutdown(&mut self) {
        println!("Let's kill QEMU!");
        self.process.kill().unwrap();
        self.wait();
    }

    //创建工作路径
    pub fn prepare_workdir(workdir: &str, seed_path: Option<String>) {
        Self::clear_workdir(workdir);
        let folders = vec![
            "/corpus/normal",
            "/metadata",
            "/corpus/crash",
            "/corpus/kasan",
            "/corpus/timeout",
            "/bitmaps",
            "/imports",
            "/seeds",
            "/snapshot",
            "/forced_imports",
        ];

        for folder in folders.iter() {
            fs::create_dir_all(format!("{}/{}", workdir, folder))
                .expect("couldn't initialize workdir");
        }
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{}/filter", workdir))
            .unwrap();
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{}/page_cache.lock", workdir))
            .unwrap();
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{}/page_cache.dump", workdir))
            .unwrap();
        OpenOptions::new()
            .create(true)
            .write(true)
            .open(format!("{}/page_cache.addr", workdir))
            .unwrap();

        OpenOptions::new().create(true).write(true).open(format!("{}/program", workdir)).unwrap();

        //println!("IMPORT STUFF FOR {:?}", seed_path);
        if let Some(path) = seed_path {
            let pattern = format!("{}/*", path);
            //println!("IMPORT STUFF FOR {}", pattern);
            for (i,p) in glob::glob(&pattern).expect("couldn't glob seed pattern??").enumerate()
            {
                let src = p.unwrap_or_else(|e| panic!("invalid seed path found {:?}",e));
                //println!("import {} to {}/seeds/seed_{}",src.to_string_lossy(), workdir,i);
                let dst = format!("{}/seeds/seed_{}.bin",workdir, i);
                fs::copy(&src, &dst).unwrap_or_else(|e| panic!("couldn't copy seed {} to {} {:?}",src.to_string_lossy(),dst,e));
            }
        }
    }

    //创建redqueen的工作路径
    fn prepare_redqueen_workdir(workdir: &str, qemu_id: usize) {
        //println!("== preparing RQ folder: {}", qemu_id);
        fs::create_dir_all(format!("{}/redqueen_workdir_{}", workdir, qemu_id))
            .expect("couldn't initialize workdir");
        //println!("== preparing RQ folder: {} DONE", qemu_id);
   
        }

    fn clear_workdir(workdir: &str) {
        let _ = fs::remove_dir_all(workdir);

        let project_name = Path::new(workdir)
            .file_name()
            .expect("Couldn't get project name from workdir!")
            .to_str()
            .expect("invalid chars in workdir path")
            .to_string();

        for p in glob::glob(&format!("/dev/shm/kafl_{}_*", project_name)).expect("couldn't glob??")
        {
            fs::remove_file(p.expect("invalid path found")).unwrap();
        }
    }
}
