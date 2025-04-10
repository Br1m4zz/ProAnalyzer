extern crate config;
extern crate core_affinity;
extern crate fuzz_runner;
extern crate helpers;
extern crate serde;
extern crate structured_fuzzer;
extern crate serde_derive;
extern crate rmp_serde;
extern crate ron;
extern crate rand;
extern crate glob;


use analyzer::SegmentAnalyzer;

use structured_fuzzer::graph_mutator::spec_loader;

use std::process;
use std::time::Duration;

use clap::{value_t, App, Arg};

use std::fs::File;
use std::thread;

use fuzz_runner::nyx::qemu_process_new_from_kernel;
use fuzz_runner::nyx::qemu_process_new_from_snapshot;
use fuzz_runner::nyx::qemu_process::QemuProcess;


use std::fs;

use config::{Config, FuzzRunnerConfig};



extern crate colored; // not needed in Rust 2018

mod input;
mod analyzer;
mod bitmap;
mod romu;
mod queue;
mod hash;
mod localhashmap;
use rand::thread_rng;
use crate::rand::Rng;
use crate::romu::*;
use crate::queue::Queue;
use colored::*;

fn main() {
    
    let matches = App::new("nyx")
        .about("Fuzz EVERYTHING!")
        .arg(// 目标打包目录
            Arg::with_name("sharedir")
                .short("s")
                .long("sharedir")
                .value_name("SHAREDIR_PATH")
                .takes_value(true)
                .help("path to the sharedir"),
        )
        .arg(
            Arg::with_name("cpu_start")
                .short("c")
                .long("cpu")
                .value_name("CPU_START")
                .takes_value(true)
                .help("overrides the config value for the first CPU to pin threads to"),
        )
        .arg(
            Arg::with_name("target_file")
                .short("f")
                .long("target_file")
                .value_name("TARGET")
                .takes_value(true)
                .help("specifies one target file"),
        )
        .arg(//要复现的测试用例的目录
            Arg::with_name("target_path")
                .short("d")
                .long("target_path")
                .value_name("TARGET_PATH")
                .takes_value(true)
                .help("specifies path to a target folder"),
        )
        .arg(//使用python打包转译好的payload
            Arg::with_name("dump_payload_folder")
                .short("t")
                .long("dump_payload_folder")
                .value_name("DUMP_PAYLOAD_PATH")
                .takes_value(true)
                .help("dump payload files to folder"),
        )
        .arg(//使用python打包转译好的payload
            Arg::with_name("workdir")
                .short("w")
                .long("workdir")
                .value_name("WORKDIR")
                .takes_value(true)
                .help("workdir"),
        )
        .arg(
            Arg::with_name("quiet")
                .short("q")
                .long("quiet")
                .takes_value(false)
                .help("quite mode - don't output aux buffer results"),
        )
        .after_help("Example: cargo run --release -- -s <SHAREDIR>  -t <OUTPUT_FOLDER>\n")
        .get_matches();

    //println!("{:?}", matches);

    let sharedir = matches
        .value_of("sharedir")
        .expect("need to specify sharedir (-s)")
        .to_string();
    
    // if !matches.value_of("target_file").is_some() && !matches.value_of("target_path").is_some() {
    //     panic!("Neither a target_file nor a target_path has been specififed!");
    // }

    let cfg: Config = Config::new_from_sharedir(&sharedir);


    let mut config = cfg.fuzz;
    let config_runner = cfg.runner;

    if let Ok(start_cpu_id) = value_t!(matches, "cpu_start", usize) {
        config.cpu_pin_start_at = start_cpu_id;
    }

    

    //println!("DUMP: {}", matches.value_of("dump_payload_folder").is_some());
    config.dump_python_code_for_inputs = Some(matches.value_of("dump_payload_folder").is_some());

    if config.dump_python_code_for_inputs.unwrap(){
        fs::create_dir_all(matches.value_of("dump_payload_folder").unwrap()).unwrap();
    }

    
    if let Some(path) = matches.value_of("workdir") {
        config.workdir_path = path.to_string();
    }else{
        config.workdir_path = format!("/tmp/calibrate_workdir_{}/", config.cpu_pin_start_at);
    }
    // let sdir = sharedir.clone();

    let specfile = File::open(&config.spec_path).expect(&format!(
        "couldn't open spec (File not found: {}",
        config.spec_path
    ));

    let spec = spec_loader::load_spec_from_read(specfile);
    let queue = Queue::new(&config);
    let timeout = config.time_limit;
    println!("timeout:{:?}",timeout);

    let mut thread_handles = vec![];    // 线程管理
    let core_ids = core_affinity::get_core_ids().unwrap();
    let seed = value_t!(matches, "cpu_start", u64).unwrap_or(thread_rng().gen());
    let mut rng = RomuPrng::new_from_u64(seed);
    QemuProcess::prepare_workdir(&config.workdir_path, config.seed_path.clone());

    

    for i in 0..config.threads {
        let mut cfg = config.clone();
        cfg.thread_id = i;

        let spec1 = spec.clone();
        let queue1 = queue.clone(); //每次新建一个queue的拷贝
        let core_id = core_ids[(i + cfg.cpu_pin_start_at) % core_ids.len()].clone();
        let thread_seed = rng.next_u64();
        let sdir = sharedir.clone();

        match config_runner.clone() {

            FuzzRunnerConfig::QemuKernel(run_cfg) => {
                thread_handles.push(thread::spawn(move ||{
                    
                    core_affinity::set_for_current(core_id);  
                    let mut runner = qemu_process_new_from_kernel(sdir, &run_cfg, &cfg);
                    runner.set_timeout(cfg.time_limit); // 设置超时
                    //runner.aux.config.page_dump_mode = 1;
                    //runner.aux.config.changed = 1;
                    let mut analyzer = SegmentAnalyzer::new(runner, cfg, spec1,queue1,thread_seed);
                    analyzer.run();
                    analyzer.shutdown();
                    println!("[!] analyzer: FINISH!");
                    process::exit(0);
                // execute(&mut runner, &matches, quite_mode, &config_fuzzer.workdir_path,spec);
                }))
            }
            
            FuzzRunnerConfig::QemuSnapshot(run_cfg) => {
                thread_handles.push(thread::spawn(move ||{
                println!("[!] fuzzer: spawning qemu instance #{}", i);  // 打印信息
                core_affinity::set_for_current(core_id);   
                let mut runner = qemu_process_new_from_snapshot(sdir, &run_cfg, &cfg);
                runner.set_timeout(cfg.time_limit); // 根据config设置超时
                let mut analyzer = SegmentAnalyzer::new(runner, cfg, spec1,queue1,thread_seed);
                // execute(&mut runner, &matches, quite_mode, &config_fuzzer.workdir_path,spec);
                analyzer.run();
                println!("[!] analyzer: FINISH!");
                process::exit(0);
                }));
                std::thread::sleep(Duration::from_millis(100));  // 线程休眠一段时间
            }
        }
    }
    //  根据具体的运行模式新建runer，开始测试

    thread_handles.push(thread::spawn(move || {
        // let mut num_bits_last = 0;
    
    loop {
            let total_execs = queue.get_total_execs();

            
            if total_execs > 0 {
                // let num_bits = queue.num_bits();

                println!("[!] {}", format!("Execs/sec: {}, Time:{}s, total_execs:{}", total_execs as f32 / queue.get_runtime_as_secs_f32(),queue.get_runtime_as_secs_f32(),total_execs).yellow().bold()); 
                // std::fs::write(
                //     &format!(
                //     "{}/tree.dot",
                //     config.workdir_path,
                //     ),
                //     queue.print_snap_tree()
                // ).unwrap();
    
            }
            std::thread::sleep(Duration::from_millis(1000*60));
        }
    }));
    for t in thread_handles.into_iter() {
        t.join().unwrap();
    }

}


