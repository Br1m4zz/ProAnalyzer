# libnyx

<p>
<img align="right" width="200"  src="logo.png">
</p>

libnyx is a library that allows you to simply build hypervisor based snapshot fuzzers. Using libnyx, managing multiple vms and snapshots as well as the communication with the code running in the VM becomes a matter of a handful of lines of code. At the moment, libnyx can be used via a simple C interface or from a rust library.

libnyx 实现了一个rust库，可让您轻松构建基于虚拟机管理程序的快照fuzzer。 使用 libnyx，管理多个虚拟机和快照以及与虚拟机中运行的被测目标的通信只需几行代码即可。 目前，libnyx 可以通过简单的 C 接口或 Rust 库来使用。


## 文件架构
```
├── acat                           //实现了一个命令行工具，它用于调试辅助缓冲区（aux buffers）
│   ├── Cargo.toml
│   └── src
│       └── main.rs
├── config                          //读取快照拍摄的配置，以及模糊测试配置的模块
│   ├── Cargo.toml
│   └── src
│       ├── config.rs
│       ├── lib.rs
│       └── loader.rs
├── fuzz_runner
│   ├── Cargo.toml
│   └── src
│       ├── exitreason.rs               //定义与实现一次运行实例退出的原因
│       ├── forksrv                     //原本的forksrv架构，现已抛弃
│       │   ├── error.rs
│       │   ├── mod.rs
│       │   └── newtypes.rs
│       ├── lib.rs                      //FuzzRuner的具体实现（nyx-net使用qemu_process实现），如何执行、调用其他分析模块
│       └── nyx
│           ├── aux_buffer.rs           //实现aux_buffer的创建，用于与模糊测试器等交互
│           ├── ijon_data.rs            //定义了ijon需要的结构体，存储和传递测试过程中收集的信息
│           ├── mem_barrier.rs          //与编译器共享内存相关的
│           ├── mod.rs                  //定义的对外创建qemu进程的方法qemu_process_new_from_kernel与qemu_process_new_from_snapshot，最终对象抽象是QemuProcess
│           ├── params.rs               //QemuParams的构造函数配置qemu进程的创建参数（快照、内核）
│           ├── qemu_process.rs         //qemuProcess具体的构建
│           └── tests
│               └── tests.rs
├── libnyx                              //面向C库接口的libnyx
│   ├── src                             //libnyx的rust实现，并包含对应的C接口
│   │   └── lib.rs
│   ├── build.rs                        //自动生成C接口库
│   ├── Cargo.toml                      //开发toml文件
│   ├── cbindgen.toml                   //cbingen的配置文件，头文件就是libnyx_h
│   ├── test.c                          //C接口测试
│   └── test.sh                         //一键编译libnyx的rust库（编译成liblibnyx.a）与C接口测试文件(编译成app)，
├── logo.png
└── README.md
```


## Bug Reports and Contributions

If you found and fixed a bug on your own: We are very open to patches, please create a pull request!  

### License

This library is provided under **AGPL license**. 

**Free Software Hell Yeah!** 

Proudly provided by: 
* [Sergej Schumilo](http://schumilo.de) - sergej@schumilo.de / [@ms_s3c](https://twitter.com/ms_s3c)
* [Cornelius Aschermann](https://hexgolems.com) - cornelius@hexgolems.com / [@is_eqv](https://twitter.com/is_eqv)
# aux buffer
为了与模糊测试工具等进行信息交互，实现的共享内存

共享内存有如下字段：
```
//header:魔术教研字
pub struct auxilary_buffer_header_s {
    pub magic: u64, /* 0x54502d554d4551 */
    pub version: u16,
    pub hash: u16,
}

//cap：功能性的buffer，与某些测试模块相关
pub struct auxilary_buffer_cap_s {
    pub redqueen: u8,
    pub agent_timeout_detection: u8,   
    pub agent_trace_bitmap: u8,  
    pub agent_ijon_trace_bitmap: u8,  
}

//config：一些模块的配置信息
pub struct auxilary_buffer_config_s {
    pub changed: u8, 
    pub timeout_sec: u8,
    pub timeout_usec: u32,
    pub redqueen_mode: u8,
    pub trace_mode: u8,
    pub reload_mode: u8,
    pub verbose_level: u8,
    pub page_dump_mode: u8,
    pub page_addr: u64,
    pub protect_payload_buffer: u8,
    pub discard_tmp_snapshot: u8,
}

//result：测试的最终结果
pub struct auxilary_buffer_result_s {
    pub state: u8,
    pub tmp_snapshot_created: u8,
    pub padding_1: u8,
    pub padding_2: u8,
    pub bb_coverage: u32,
    pub padding_3: u8,
    pub padding_4: u8,

    pub hprintf: u8,
    pub exec_done: u8,
    pub crash_found: u8,
    pub asan_found: u8,
    pub timeout_found: u8,
    pub reloaded: u8,
    pub pt_overflow: u8,

    pub runtime_sec: u8,

    pub page_not_found: u8,
    pub success: u8,
    pub runtime_usec: u32,
    pub page_not_found_addr: u64,
    pub dirty_pages: u32,
    pub pt_trace_size: u32, 
    pub payload_write_attempt_found: u8,

}
//misc：杂项数据，并提供一些方法显示这些数据
pub struct auxilary_buffer_misc_s {
    pub len: u16,
    pub data: [u8;MISC_SIZE-2],
}
```


# lib_nyx实现并提供的C接口
核心围绕着qemu_process这个对象的操作
## nyx_new
nyx_new(sharedir: *const c_char, workdir: *const c_char, worker_id: u32, create_snapshot: bool) -> * mut QemuProcess

根据提供的目录路径和配置，创建并初始化一个新的QEMU虚拟机进程:
具体调用的是`qemu_process_new_from_snapshot` 或者`qemu_process_new_from_kernel`

## nyx_get_aux_buffer
nyx_get_aux_buffer(qemu_process: * mut QemuProcess) -> *mut u8

从一个QemuProcess结构体实例中获取一个指向其辅助缓冲区（auxiliary buffer）的原始指针

直接指向`((*qemu_process).aux.header.magic)`

## nyx_get_payload_buffer
nyx_get_payload_buffer(qemu_process: * mut QemuProcess) -> *mut u8

获取qemu进程的payload指针

直接指向`(*qemu_process).payload.as_mut_ptr()`
## nyx_get_bitmap_buffer
nyx_get_bitmap_buffer(qemu_process: * mut QemuProcess) -> *mut u8

获取qemu进程的bitmap指针
## nyx_shutdown
nyx_shutdown(qemu_process: * mut QemuProcess)

杀死目标QEMU虚拟机进程：

直接调用：`(*qemu_process).shutdown()`

## nyx_option_set_reload_mode
nyx_option_set_reload_mode(qemu_process: * mut QemuProcess, enable: bool)

设置虚拟机的重载模式：

## nyx_option_set_timeout
nyx_option_set_timeout(qemu_process: * mut QemuProcess, timeout_sec: u8, timeout_usec: u32)

设置虚拟机的超时时间：


## nyx_option_apply
nyx_option_apply(qemu_process: * mut QemuProcess)


## nyx_exec
nyx_exec(qemu_process: * mut QemuProcess) -> NyxReturnValue

设置qemu内进程执行

## nyx_set_afl_input
nyx_set_afl_input(qemu_process: * mut QemuProcess, buffer: *mut u8, size: u32)

将NYX的AFL的输入buffer传入到payload中：


## nyx_print_aux_buffer
nyx_print_aux_buffer(qemu_process: * mut QemuProcess)

打印qemu进程的辅助缓冲区的result调试信息：

# 重要对象

## Config相关的：
基本上都是使用new_from_loader创建这些对象的实例
### QemuKernelConfig
qemu运行配置

### QemuSnapshotConfig
qemu的快照运行配置

### FuzzRunnerConfig
通过FuzzRunner对象管理以上的配置（QemuSnapshotConfig、QemuKernelConfig）

### FuzzerConfig
用于配置模糊测试工具的参数。

### Config
存取FuzzerConfig和FuzzRunnerConfig的配置。既可以使用配置加载器创建，也可以使用共享目录创建（/config.ron）