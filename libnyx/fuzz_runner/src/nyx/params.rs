use std::path::Path;
use crate::config::SnapshotPath;

pub struct KernelVmParams { //构建新内核虚拟机的参数
    pub qemu_binary: String,
    pub kernel: String,     //内核特有
    pub sharedir: String,
    pub ramfs: String,      //内核特有
    pub ram_size: usize,
    pub bitmap_size: usize,
    pub debug: bool,

    pub dump_python_code_for_inputs: bool,
}

pub struct SnapshotVmParams{    //构建快照的参数
    pub qemu_binary: String,
    pub hda: String,    //快照特有
    pub sharedir: String,
    pub presnapshot: String,    //前置快照
    pub snapshot_path: SnapshotPath,    //快照的路径
    pub ram_size: usize,
    pub bitmap_size: usize,
    pub debug: bool,

    pub dump_python_code_for_inputs: bool,
}

pub struct QemuParams {
    pub cmd: Vec<String>,
    pub qemu_aux_buffer_filename: String,
    pub control_filename: String,   //qemu的通信unix stream
    pub bitmap_filename: String,    //bitmap文件名称
    pub payload_filename: String,   //payload文件名
    pub binary_filename: String,    //二进制
    pub workdir: String,
    pub qemu_id: usize,
    pub bitmap_size: usize,
    pub payload_size: usize,

    pub dump_python_code_for_inputs: bool,
}

impl QemuParams {
    //构造函数1：基于快照新建qemu进程（5参数）
    pub fn new_from_snapshot(workdir: &str, qemu_id: usize, cpu: usize, params: &SnapshotVmParams, create_snapshot_file: bool) -> QemuParams{
    
        assert!(!(!create_snapshot_file && qemu_id == 1));
        let project_name = Path::new(workdir)
        .file_name()
        .expect("Couldn't get project name from workdir!")
        .to_str()
        .expect("invalid chars in workdir path")
        .to_string();

        let qemu_aux_buffer_filename = format!("{}/aux_buffer_{}", workdir, qemu_id);
        let payload_filename = format!("/dev/shm/kafl_{}_qemu_payload_{}", project_name, qemu_id);
        //let tracedump_filename = format!("/dev/shm/kafl_{}_pt_trace_dump_{}", project_name, qemu_id);
        let binary_filename = format!("{}/program", workdir);
        let bitmap_filename = format!("/dev/shm/kafl_{}_bitmap_{}", project_name, qemu_id);
        let control_filename = format!("{}/interface_{}", workdir, qemu_id);
        
        //配置启动的命令行参数
        let mut cmd = vec![];
        cmd.push(params.qemu_binary.to_string());

        cmd.push("-drive".to_string()); //指定虚拟机的硬盘镜像文件。
        cmd.push(format!("file={},format=raw,index=0,media=disk", params.hda.to_string()));

        if !params.debug {
            cmd.push("-display".to_string());
            cmd.push("none".to_string());
        } else {
            cmd.push("-vnc".to_string());
            cmd.push(format!(":{}",qemu_id+cpu));
        }

        cmd.push("-serial".to_string());
        if params.debug {
            cmd.push("mon:stdio".to_string());
        } else {
            cmd.push("stdio".to_string());//调试模式，将其设置为监视器模式
        }

        cmd.push("-enable-kvm".to_string()); //启用KVM加速。

        cmd.push("-net".to_string()); //禁用网络功能。
        cmd.push("none".to_string());

        cmd.push("-k".to_string()); //设置键盘布局为德语
        cmd.push("de".to_string());

        cmd.push("-m".to_string()); //设置虚拟机的内存大小
        cmd.push(params.ram_size.to_string());

        cmd.push("-chardev".to_string());   //创建一个字符设备，用于与虚拟机通信
        cmd.push(format!(
            "socket,server,path={},id=kafl_interface",
            control_filename
        ));

        //qemu拍摄快照的执行参数大概长这样：
        // -fast_vm_reload path=/tmp/fuzz_workdir/snapshot/,load=off,pre_path=/home/kafl/ubuntu_snapshot
    
        cmd.push("-device".to_string());    //选项添加一个自定义设备，这里是kafl设备，用于模糊测试
        let mut nyx_ops = format!("kafl,chardev=kafl_interface");
        nyx_ops += &format!(",bitmap_size={}", params.bitmap_size+0x1000);
        nyx_ops += &format!(",worker_id={}", qemu_id);
        nyx_ops += &format!(",workdir={}", workdir);
        nyx_ops += &format!(",sharedir={}", params.sharedir);
        //nyx_ops += &format!(",ip0_a=0x1000,ip0_b=0x7ffffffff000");
        //nyx_ops += &format!(",ip0_a=ffff800000000000,ip0_b=ffffffffffffffff");

        cmd.push(nyx_ops);

        cmd.push("-machine".to_string());   //使用-machine设置虚拟机的类型。
        cmd.push("kAFL64-v1".to_string());

        cmd.push("-cpu".to_string());   //使用-cpu设置cpu型号。
        cmd.push("kAFL64-Hypervisor-v1".to_string());
        //cmd.push("kvm64-v1,".to_string());

        match &params.snapshot_path {   //根据snapshot_path配置QEMU虚拟机启动时的快照加载行为
            SnapshotPath::Create(path) => { //创建快照
                if create_snapshot_file {       //根据create_snapshot_file的数值决定配置参数
                    cmd.push("-fast_vm_reload".to_string());
                    cmd.push(format!("path={},load=off,pre_path={}", path,params.presnapshot));
                }
                else{
                    cmd.push("-fast_vm_reload".to_string());
                    cmd.push(format!("path={},load=off,pre_path={},skip_serialization=on", path,params.presnapshot));
                }
            },
            SnapshotPath::Reuse(path) => { //重用快照
                cmd.push("-fast_vm_reload".to_string());
                cmd.push(format!("path={},load=on", path));
            }
            SnapshotPath::DefaultPath => panic!(), //异常
        }
    
        /*
        bin = read_binary_file("/tmp/zsh_fuzz")
        assert(len(bin)<= (128 << 20))
        atomic_write(binary_filename, bin)
        */
        return QemuParams {
            cmd,
            qemu_aux_buffer_filename,
            control_filename,
            bitmap_filename,
            payload_filename,
            binary_filename,
            workdir: workdir.to_string(),
            qemu_id,
            bitmap_size: params.bitmap_size,
            payload_size: (1 << 16),
            dump_python_code_for_inputs: params.dump_python_code_for_inputs,
        };
    }

    //构造函数2：基于内核新建qemu进程（4参数）
    pub fn new_from_kernel(workdir: &str, qemu_id: usize, params: &KernelVmParams, create_snapshot_file: bool) -> QemuParams {
        //prepare_working_dir(workdir)

        assert!(!(!create_snapshot_file && qemu_id == 1));
        let project_name = Path::new(workdir)
            .file_name()
            .expect("Couldn't get project name from workdir!")
            .to_str()
            .expect("invalid chars in workdir path")
            .to_string();

        let qemu_aux_buffer_filename = format!("{}/aux_buffer_{}", workdir, qemu_id);
        let payload_filename = format!("/dev/shm/kafl_{}_qemu_payload_{}", project_name, qemu_id);
        //let tracedump_filename = format!("/dev/shm/kafl_{}_pt_trace_dump_{}", project_name, qemu_id);
        let binary_filename = format!("{}/program", workdir);
        let bitmap_filename = format!("/dev/shm/kafl_{}_bitmap_{}", project_name, qemu_id);
        let control_filename = format!("{}/interface_{}", workdir, qemu_id);

        let mut cmd = vec![];

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
    //    -fast_vm_reload path=[workdir]/snapshot/,load=off[或者on]
        cmd.push(params.qemu_binary.to_string());
        cmd.push("-kernel".to_string());
        cmd.push(params.kernel.to_string());

        cmd.push("-initrd".to_string());
        cmd.push(params.ramfs.to_string());

        cmd.push("-append".to_string());
        cmd.push("nokaslr oops=panic nopti ignore_rlimit_data".to_string());

        if !params.debug {
            cmd.push("-display".to_string());
            cmd.push("none".to_string());
        }

        cmd.push("-serial".to_string());
        if params.debug {
            cmd.push("mon:stdio".to_string());
        } else {
            cmd.push("none".to_string());
        }

        cmd.push("-enable-kvm".to_string());

        cmd.push("-net".to_string());
        cmd.push("none".to_string());

        cmd.push("-k".to_string());
        cmd.push("de".to_string());

        cmd.push("-m".to_string());
        cmd.push(params.ram_size.to_string());

        //cmd.push//("-cdrom".to_string());
        //cmd.push("/home/kafl/rust_dev/nyx/syzkaller_spec/cd.iso".to_string());

        cmd.push("-chardev".to_string());
        cmd.push(format!(
            "socket,server,path={},id=kafl_interface",
            control_filename
        ));

        cmd.push("-device".to_string());
        let mut nyx_ops = format!("kafl,chardev=kafl_interface");
        nyx_ops += &format!(",bitmap_size={}", params.bitmap_size+0x1000); /* + ijon page */
        nyx_ops += &format!(",worker_id={}", qemu_id);
        nyx_ops += &format!(",workdir={}", workdir);
        nyx_ops += &format!(",sharedir={}", params.sharedir);

        //nyx_ops += &format!(",ip0_a=0x1000,ip0_b=0x7ffffffff000");
        //nyx_ops += &format!(",ip0_a=ffff800000000000,ip0_b=ffffffffffffffff");

        cmd.push(nyx_ops);

        cmd.push("-machine".to_string());
        cmd.push("kAFL64-v1".to_string());

        cmd.push("-cpu".to_string());
        cmd.push("kAFL64-Hypervisor-v1,+vmx".to_string());
        //cmd.push("kvm64-v1,+vmx".to_string());

        if create_snapshot_file {
            cmd.push("-fast_vm_reload".to_string());
            if qemu_id == 0{
                cmd.push(format!("path={}/snapshot/,load=off", workdir));
            } else {
                cmd.push(format!("path={}/snapshot/,load=on", workdir));
            }
        }

        /*
        bin = read_binary_file("/tmp/zsh_fuzz")
        assert(len(bin)<= (128 << 20))
        atomic_write(binary_filename, bin)
        */
        return QemuParams {
            cmd,
            qemu_aux_buffer_filename,
            control_filename,
            bitmap_filename,
            payload_filename,
            binary_filename,
            workdir: workdir.to_string(),
            qemu_id,
            bitmap_size: params.bitmap_size,
            payload_size: (128 << 10),
            dump_python_code_for_inputs: params.dump_python_code_for_inputs,
        };
    }
}
