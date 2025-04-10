pub mod aux_buffer;
pub mod ijon_data;
pub mod mem_barrier;
pub mod params;
pub mod qemu_process;

pub use qemu_process::QemuProcess;

use std::fs;
use std::path::PathBuf;

extern crate config;
use crate::config::{QemuKernelConfig, QemuSnapshotConfig, FuzzerConfig, SnapshotPath};

fn into_absolute_path(sharedir: &str) -> String{

    let srcdir = PathBuf::from(&sharedir);

    if srcdir.is_relative(){
        return fs::canonicalize(&srcdir).unwrap().to_str().unwrap().to_string();
    }
    else{
        return sharedir.to_string();
    }
}

//从kernel新建qemu进程
pub fn qemu_process_new_from_kernel(sharedir: String, cfg: &QemuKernelConfig, fuzz_cfg: &FuzzerConfig) -> qemu_process::QemuProcess {
    let params = params::KernelVmParams {
        qemu_binary: cfg.qemu_binary.to_string(),
        kernel: cfg.kernel.to_string(),
        sharedir: into_absolute_path(&sharedir),
        ramfs: cfg.ramfs.to_string(),
        ram_size: fuzz_cfg.mem_limit,
        bitmap_size: fuzz_cfg.bitmap_size,
        debug: cfg.debug,
        dump_python_code_for_inputs: match fuzz_cfg.dump_python_code_for_inputs{
            None => false,
            Some(x) => x,
        }
    };
    let qemu_id =  fuzz_cfg.thread_id;
    let qemu_params = params::QemuParams::new_from_kernel(&fuzz_cfg.workdir_path, qemu_id, &params, fuzz_cfg.threads > 1);
   
    /*
    if qemu_id == 0{
        qemu_process::QemuProcess::prepare_workdir(&fuzz_cfg.workdir_path, fuzz_cfg.seed_pattern.clone());
    }
    */
    return qemu_process::QemuProcess::new(qemu_params);
}

///生成基于快照的qemu进程实例。
/// 具体的配置快照路径、生成qemu对应的运行参数、生成qemu执行实例。
/// 
/// qemu_id对应启动的线程id
/// 
/// 返回准备好模糊测试的qemu执行实例
pub fn qemu_process_new_from_snapshot(sharedir: String, cfg: &QemuSnapshotConfig,  fuzz_cfg: &FuzzerConfig) -> qemu_process::QemuProcess {
    //根据snapshot_path的类型创建不同的测试用例执行配置
    let snapshot_path = match &cfg.snapshot_path{
        SnapshotPath::Create(_x) => panic!(),           //如果要调用创建快照，则抛出异常
        SnapshotPath::Reuse(x) => SnapshotPath::Reuse(x.to_string()),   //重用现有的快照
        SnapshotPath::DefaultPath => {  //配置是默认路径
            //如果fuzz配置的thread_id值为0,那么就根据fuzz_cfg的工作路径配置快照创建
            if fuzz_cfg.thread_id == 0 {
                SnapshotPath::Create(format!("{}/snapshot/",fuzz_cfg.workdir_path))
            //其他情况则就利用现有的快照
            } else {
                SnapshotPath::Reuse(format!("{}/snapshot/",fuzz_cfg.workdir_path))
            }
        }
    };
    //qemu新建内核的执行参数大概长这样：
    // qemu-system-x86_64 
    //    -kernel [kernel目标]
    //    -initrd [ramfs]
    //    -append [nokaslr oops=panic nopti ignore_rlimit_data]
    //    -display none
    //    -serial mon:stdio none
    //    -enable-kvm
    //    -net none
    //    -k de
    //    -m [ram_size]
    //    -chardev socket,server,path=[control_filename],id=kafl_interface
    //    -device kafl,chardev=kafl_interface,bitmap_size=[bitmap大小],worker_id=[qemu_id],workdir=[workdir],sharedir=[]
    //    -machine kAFL64-v1
    //    -cpu kAFL64-Hypervisor-v1,+vmx
    //    根据create_snapshot_file的额外添加参数：
    //      -fast_vm_reload path=[workdir]/snapshot/,load=off[或者on]

    // 根据传入的参数设置QEMU-VM的参数，生成最后qemu执行的参数（qemu_params）
    let params = params::SnapshotVmParams {
        qemu_binary: cfg.qemu_binary.to_string(),
        hda: cfg.hda.to_string(),
        sharedir: into_absolute_path(&sharedir),
        presnapshot: cfg.presnapshot.to_string(),   // 预快照名称
        ram_size: fuzz_cfg.mem_limit,
        bitmap_size: fuzz_cfg.bitmap_size,
        debug: cfg.debug,
        snapshot_path,
        dump_python_code_for_inputs: match fuzz_cfg.dump_python_code_for_inputs{
            None => false,
            Some(x) => x,
        }
    };
    let qemu_id = fuzz_cfg.thread_id;
    let qemu_params = params::QemuParams::new_from_snapshot(&fuzz_cfg.workdir_path, qemu_id, fuzz_cfg.cpu_pin_start_at, &params, fuzz_cfg.threads > 1);
    
    /*
    if qemu_id == 0{
        println!("------> WIPING EVERYTHING");
        qemu_process::QemuProcess::prepare_workdir(&fuzz_cfg.workdir_path, fuzz_cfg.seed_pattern.clone());
        println!("------> WIPING EVERYTHING DONE");
    }
    */

    //根据qemu_params，生成对应的qemu实例
    return qemu_process::QemuProcess::new(qemu_params);
}

//测试用
#[cfg(test)]
mod tests {
    //use crate::aux_buffer::*;
    use super::params::*;
    use super::qemu_process::*;
    //use std::{thread, time};

    #[test]
    fn it_works() {
        let workdir = "/tmp/workdir_test";
        let params = KernelVmParams {
            qemu_binary: "/home/kafl/NEW2/QEMU-PT_4.2.0/x86_64-softmmu/qemu-system-x86_64"
                .to_string(),
            kernel: "/home/kafl/Target-Components/linux_initramfs/bzImage-linux-4.15-rc7"
                .to_string(),
            ramfs: "/home/kafl/Target-Components/linux_initramfs/init.cpio.gz".to_string(),
            sharedir: "foo! invalid".to_string(),
            ram_size: 1000,
            bitmap_size: 0x1 << 16,
            debug: false,
            dump_python_code_for_inputs: false,
        };
        let qemu_id = 1;
        let qemu_params = QemuParams::new_from_kernel(workdir, qemu_id, &params);

        QemuProcess::prepare_workdir(&workdir, None);

        let mut qemu_process = QemuProcess::new(qemu_params);

        for _i in 0..100 {
            qemu_process.send_payload();
        }
        println!("test done");
    }
}
