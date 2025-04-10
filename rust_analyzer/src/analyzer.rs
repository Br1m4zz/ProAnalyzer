
use crate::bitmap::{Bitmap, StorageReason};
use crate::fuzz_runner::FuzzRunner;
use crate::fuzz_runner::{ExitReason, TestInfo};
use crate::input::Input;

use crate::localhashmap::LocalHashmap;
use crate::queue::Queue;
use crate::structured_fuzzer::graph_mutator::graph_storage::{RefGraph, VecGraph};
use crate::structured_fuzzer::graph_mutator::spec::GraphSpec;
use crate::structured_fuzzer::mutator::{Mutator, MutatorSnapshotState};
use crate::structured_fuzzer::random::distributions::Distributions;
use crate::structured_fuzzer::GraphStorage;
use crate::structured_fuzzer::mutator::MutationStrategy;

use crate::config::FuzzerConfig;

//use std::error::Error;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
//use std::io::Write;


extern crate colored; // not needed in Rust 2018

use serde_json;
use structured_fuzzer::mutator::DetMutator;
use crate::{hash, romu::*};
use colored::*;
use std::str;
use std::fmt; // 引入正确的 trait
pub trait GetStructStorage {
    fn get_struct_storage(&mut self, checksum: u64) -> RefGraph;
}

impl<T: FuzzRunner> GetStructStorage for T {
    ///根据当前模糊测试传入的输入input，更新该input的头部的checksum，获取模糊测试用的refgraph
    fn get_struct_storage(&mut self, checksum: u64) -> RefGraph {
        return RefGraph::new_from_slice(self.input_buffer(), checksum);
    }
}

use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize)]
struct PacketCalibrationResult {
    packet_id: usize,             // 包编号
    offset: usize,                // 偏移量
    stable: bool,
    mutation_operator: String,    // 使用的变异算子
    cf_index: usize,        // CF 索引
    vf_index: usize,        // VF 索引
    cfc_index:usize,        //有bucket信息的索引    
}

#[derive(Serialize, Deserialize)]
struct SequenceCalibrationResults {
    sequence_id: usize,                      // 序列编号
    cal_time: f32,
    pkt_number:usize,
    raw_data: Option<String>,       // 整个包序列的摘要或统计信息
    packets_cali_result: Vec<PacketCalibrationResult>,  // 每个包的测量结果
}


pub struct SegmentAnalyzer<Fuzz: FuzzRunner + GetStructStorage> {
    fuzzer: Fuzz,                                   //fuzzer管理器
    queue: Queue,                                   //测试用例队列管理器
    master_rng: RomuPrng,                           //随机数生成器
    rng: Distributions,                             //变异概率分布管理器
    mutator: Mutator,                               //spec变异器
    det_mutator: DetMutator,                        //spec 确定性变异器
    localhashmap: LocalHashmap,                         //bitmap管理器，记录全局的bitmap
    config: FuzzerConfig,                           //fuzz配置
}

impl<Fuzz: FuzzRunner + GetStructStorage> SegmentAnalyzer<Fuzz> {
    pub fn new(fuzzer: Fuzz, config: FuzzerConfig, spec: GraphSpec,queue: Queue,seed:u64) -> Self {
        let rng = Distributions::new(config.dict.clone());//根据字典构建随机变异器

        //基于specfuzz需要的变异变异算子
        let mutator = Mutator::new(spec.clone());
        let det_mutator = DetMutator::new(spec.clone());
        //创建模糊测试需要记录的bitmap管理句柄、随机数生成器、模糊测试统计信息
        let localhashmap = LocalHashmap::new();
        let master_rng = RomuPrng::new_from_u64(seed);

        //配置后续的文件的处理方式：有则打开可读可写，没有则创建
        let mut option = OpenOptions::new();
        option.read(true);
        option.write(true);
        option.create(true);
        return Self {
            fuzzer,
            queue,
            master_rng,
            rng,
            mutator,
            det_mutator,
            localhashmap,
            config,
        };
    }

    // fn perform_run_get_testinfo<F>(&mut self, f: F) -> Option<(TestInfo, usize, usize,usize)>
    // where
    //     F: Fn(&mut DetMutator, &Distributions, &mut RefGraph),
    // {
    //     // 设置随机数种子
    //     let (seed_x, seed_y) = (self.master_rng.next_u64(), self.master_rng.next_u64());
    //     self.rng.set_full_seed(seed_x, seed_y);
    
    //     // 获取当前 storage
    //     let mut storage = self.fuzzer.get_struct_storage(self.mutator.spec.checksum);
    
    //     // 执行闭包变异函数 F
    //     f(&mut self.det_mutator, &self.rng, &mut storage);
    
    //     // 打印变异后的测试用例
    //     // println!(
    //     //     "=====================\nRUN mutated testcase:\n{}",
    //     //     storage.as_vec_graph().to_script(&self.mutator.spec)
    //     // );
    
    //     // 执行测试并获取结果
    //     let res = self.fuzzer.run_test();
    
    //     if let Ok(exec_res) = res {
    //         // 获取 cf_index 和 vf_index
    //         let cfc_index = self.localhashmap.handle_run_bitmap(self.fuzzer.bitmap_buffer());
    //         let cf_index = self.localhashmap.handle_cov_bitmap(self.fuzzer.bitmap_buffer());
    //         let vf_index = self.localhashmap.handle_ijon_map(self.fuzzer.ijon_max_buffer());
    
    //         // 返回 TestInfo、cf_index 和 vf_index
    //         return Some((exec_res, cf_index, vf_index,cfc_index));
    //     }
    
    //     None
    // }

    fn perform_run_get_testinfo<F>(&mut self, f: F) -> Option<(TestInfo, usize, usize, usize,bool)>
    where
        F: Fn(&mut DetMutator, &Distributions, &mut RefGraph),
    {
        // 连续相同的阈值（例如连续4次执行产生的 run_bitmap 哈希值相同，则认为稳定）
        const MIN_STABLE_RUNS: usize = 4;
        // 最大尝试次数
        const MAX_ATTEMPTS: usize = 10;     
        // 用于记录连续稳定的执行次数
        let mut stable_counter = 0;
        // 用于记录上一次的执行哈希
        let mut last_exec_hash: Option<u64> = None;
        // 保存最新一次成功执行的测试结果
        let mut exec_res_final: Option<TestInfo> = None;
        // 设置随机数种子
        let (seed_x, seed_y) = (self.master_rng.next_u64(), self.master_rng.next_u64());
        self.rng.set_full_seed(seed_x, seed_y);
        // 获取当前 storage
        let mut storage = self.fuzzer.get_struct_storage(self.mutator.spec.checksum);
        // 执行闭包变异函数 F
        f(&mut self.det_mutator, &self.rng, &mut storage);

        for _ in 0..MAX_ATTEMPTS {
            // 执行测试并获取结果
            if let Ok(exec_res) = self.fuzzer.run_test() {
                // 计算当前执行的哈希值
                let run_bitmap = self.fuzzer.bitmap_buffer();
                let cur_exec_hash = hash::hash64(run_bitmap, run_bitmap.len());
        
                // 如果是第一次执行，直接记录当前哈希值
                if last_exec_hash.is_none() {
                    last_exec_hash = Some(cur_exec_hash);
                    stable_counter = 1;
                    exec_res_final = Some(exec_res);
                } else if last_exec_hash.unwrap() == cur_exec_hash {
                    // 与上一次执行哈希值相同，稳定计数器递增
                    stable_counter += 1;
                    exec_res_final = Some(exec_res);
                } else {
                    // 如果不同，则重置计数器和记录当前哈希
                    stable_counter = 1;
                    last_exec_hash = Some(cur_exec_hash);
                    exec_res_final = Some(exec_res);
                }
        
                // 如果连续稳定次数达到预期，则认为结果稳定
                if stable_counter >= MIN_STABLE_RUNS {
                    // 稳定后，再获取最终返回需要的索引
                    let cf_index = self.localhashmap.handle_cov_bitmap(self.fuzzer.bitmap_buffer());
                    let cfc_index = self.localhashmap.handle_run_bitmap(self.fuzzer.bitmap_buffer());
                    let vf_index = self.localhashmap.handle_ijon_map(self.fuzzer.ijon_max_buffer());
                    
                    return Some((exec_res_final.unwrap(), cf_index, vf_index, cfc_index,true));
                }
            }
        }
        
        // 如果达到最大尝试次数后还未稳定，则返回最后一次的执行结果（同样重新计算索引）
        if let Some(res) = exec_res_final {
            println!("test unstable!");
            let cf_index = self.localhashmap.handle_cov_bitmap(self.fuzzer.bitmap_buffer());
            let cfc_index = self.localhashmap.handle_run_bitmap(self.fuzzer.bitmap_buffer());
            let vf_index = self.localhashmap.handle_ijon_map(self.fuzzer.ijon_max_buffer());
            return Some((res, cf_index, vf_index, cfc_index,false));
        }
        
        // 最后返回 None
        None
    }
    


    fn perform_run_import<F>(&mut self, f: F) -> Option<TestInfo>
    where
    F: Fn(&mut DetMutator,&Distributions, &mut RefGraph){

        let (seed_x, seed_y) = (self.master_rng.next_u64(), self.master_rng.next_u64());//设置随机数种子
        self.rng.set_full_seed(seed_x, seed_y);
        //获取当前storage
        let mut storage = self.fuzzer.get_struct_storage(self.mutator.spec.checksum);

        //************************************************************ */
        //执行闭包变异函数F
        f(&mut self.det_mutator,&self.rng, &mut storage);
        //************************************************************ */

        // println!("Import testcase:\n{}",storage.as_vec_graph().to_script(&self.mutator.spec));
        // println!("====== EXECUTE INPUT LENGTH {} =======",storage.as_vec_graph().node_len(&self.mutator.spec));
        //命令运行实例调度器执行测试，并获得当前的执行结果res
        let res = self.fuzzer.run_test();

        if let Ok(exec_res) = res {

            // if let Some(new_bytes) = self
            //         .queue
            //         .check_new_bytes(self.fuzzer.bitmap_buffer(), &exec_res.exitreason, MutationStrategy::SeedImport)
            //     {
                    let storagereason = vec![StorageReason::Imported];
                    let data = {
                        let storage =
                            self.fuzzer.get_struct_storage(self.mutator.spec.checksum);
                        //let strategy = f(&self.mutator, &self.queue, &self.rng, &mut storage);
                        self.mutator.dump_graph(&storage)
                    };
                                // 记录这个输入的vecGraph信息
                    // 计算data记录的vecGraph的节点长度和操作使用数量。
                    let node_len = data.node_len(&self.mutator.spec);
                    let ops_used = std::cmp::min(exec_res.ops_used as usize, node_len);
                    // 根据上述记录的信息，新建此输入对象
                    let input = Input::new(
                        data,
                        MutationStrategy::SeedImport,
                        storagereason,
                        Bitmap::new_from_buffer(self.fuzzer.bitmap_buffer()),
                        exec_res.exitreason.clone(),
                        ops_used,
                        std::time::Duration::from_millis(0),
                    );

                    if input.storage_reasons.len() > 0 {
                        //若有crash等其他原因，保存输入为文件
                        self.new_input(&input);
                        //将这个输入添加到queue中
                        self.queue.add(input, &self.mutator.spec);
                    }
                // }
            return Some(exec_res);
        }
        return None;
    }


    fn new_input(&mut self, input: &Input) {
        use std::fs;
        //use std::time::SystemTime;
        //let t = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs();
        //let t = 1;
        let input_type_colored = match input.exit_reason{
            ExitReason::Crash(_) => {
                input.exit_reason.name().red().bold()
            },
            ExitReason::InvalidWriteToPayload(_) => {
                input.exit_reason.name().yellow().bold()
            },
            ExitReason::Timeout => {
                input.exit_reason.name().yellow().bold()
            }
            _ => {
                input.exit_reason.name().clear()
            },
        };
        //打印input测试用例的执行结果信息
        println!(
            //"[{}] Thread {} Found input {} (len:{}) {}/{} new bytes by {:?}",
            //t,
            "[{}] fuzzer: found input {} (len:{}/snap:{}) {}/{} new bytes by {:?}",
            self.config.thread_id,
            input_type_colored,
            input.data.node_len(&self.mutator.spec),
            input.parent_snapshot_position,
            input.storage_reasons.iter().filter(|r| r.has_new_byte()).count(),
            input.storage_reasons.len(),
            input.found_by
        );
        //设置语料库路径
        fs::create_dir_all(&format!(
            "{}/corpus/{}",
            self.config.workdir_path,
            input.exit_reason.name()
        ))
        .unwrap();



        let id = self.queue.next_id();
        input.data.write_to_file(
            &format!(
                "{}/corpus/{}/cnt_{}.bin",
                self.config.workdir_path,
                input.exit_reason.name(),
                id
            ),
            &self.mutator.spec,
        );
        match &input.exit_reason {
            //处理crash的测试用例的log
            ExitReason::Crash(desc) => {
                std::fs::write(
                    &format!(
                        "{}/corpus//{}/{}.log",
                        self.config.workdir_path,
                        input.exit_reason.name(),
                        id,
                    ),
                    &format!("{}\n", str::from_utf8(&desc).unwrap()),
                )
                .unwrap();
            }
            //处理invalidinput的测试用例的log
            ExitReason::InvalidWriteToPayload(desc) => {//invalidinput
                std::fs::write(
                    &format!(
                        "{}/corpus//{}/{}.log",
                        self.config.workdir_path,
                        input.exit_reason.name(),
                        id
                    ),
                    desc,
                )
                .unwrap();
            }
            _ => {}
        }

        //TODO:代码还缺少做minimiz情况的处理
    }

    fn perform_import(&mut self, seed_import: bool){
        use glob::glob;

        //根据seed_import的bool值确定测试用例的获取路径
        let search_path = if !seed_import {
                format!("{}/imports/*.bin",self.config.workdir_path)
        }
        else {
            format!("{}/seeds/*.bin",self.config.workdir_path)
        };

        for entry in glob(&search_path).expect("Failed to read glob pattern") {
            if let Ok(path) = entry {
                println!("[!] fuzzer: Trying to import {:?}", path.to_str());
                let orig = VecGraph::new_from_bin_file(path.to_str().unwrap(), &self.mutator.spec);

                //完整测一下
                self.perform_run_import(|det_mutator, rng, storage| {
                    det_mutator.copy_all(&orig, storage, &rng);
                });
            }
        }
    }

    fn perform_calibrate_no_mutation(
        &mut self,
        m1_m2_vec: &VecGraph,
        snapshot_state: &MutatorSnapshotState,
    ) -> Option<(TestInfo, usize, usize,usize,bool)> {
        // 0xff 测量
        if let Some((test_info, cf_index, vf_index,cfc_index,isstable)) = self.perform_run_get_testinfo(
            |def_mutator, rng, storage| {
                // 调用 mutate_data_full_bit_flip 进行变异操作
                def_mutator.append_unmutate(m1_m2_vec, snapshot_state, storage, rng)
            },
        ) {
            // 打印测试结果和索引信息
            // println!("Orig Info: {:?}", test_info);
            // println!("CF Index: {}", cf_index);
            // println!("VF Index: {}", vf_index);
    
            // 返回三元组
            return Some((test_info, cf_index, vf_index, cfc_index, isstable));
        } else {
            // 如果测试失败或不有趣，记录相应信息
            println!("Test failed or was not interesting.");
        }
    
        None
    }

    fn perform_calibrate_full_bit_flip(
        &mut self,
        m1_m2_vec: &VecGraph,
        snapshot_state: &MutatorSnapshotState,
        offset: usize,
    ) -> Option<(TestInfo, usize, usize,usize,bool)> {
        // 0xff 测量
        if let Some((test_info, cf_index, vf_index,cfc_index,isstable)) = self.perform_run_get_testinfo(
            |def_mutator, rng, storage| {
                // 调用 mutate_data_full_bit_flip 进行变异操作
                def_mutator.mutate_data_full_bit_flip(m1_m2_vec, snapshot_state, storage, rng, offset)
            },
        ) {
            // 打印测试结果和索引信息
            // println!("Fullbit Flip Info: {:?}", test_info);
            // println!("CALIBRATE OFFSET:{}",offset);
            // println!("CF Index: {}", cf_index);
            // println!("VF Index: {}", vf_index);
            // 返回三元组
            return Some((test_info, cf_index, vf_index, cfc_index, isstable));
        } else {
            // 如果测试失败或不有趣，记录相应信息
            println!("Test failed or was not interesting.");
        }
    
        None
    }

    fn perform_calibrate_lowest_bit_flip(
        &mut self,
        m1_m2_vec: &VecGraph,
        snapshot_state: &MutatorSnapshotState,
        offset: usize,
    ) -> Option<(TestInfo, usize, usize,usize,bool)> {
        // 0xff 测量
        if let Some((test_info, cf_index, vf_index,cfc_index,isstable)) = self.perform_run_get_testinfo(
            |def_mutator, rng, storage| {
                // 调用 mutate_data_full_bit_flip 进行变异操作
                def_mutator.mutate_data_lowest_bit_flip(m1_m2_vec, snapshot_state, storage, rng, offset)
            },
        ) {
            // 打印测试结果和索引信息
            // println!("LOWbit Flip Info: {:?}", test_info);
            // println!("CALIBRATE OFFSET:{}",offset);
            // println!("CF Index: {}", cf_index);
            // println!("VF Index: {}", vf_index);
            // 返回三元组
            return Some((test_info, cf_index, vf_index, cfc_index, isstable));
        } else {
            // 如果测试失败或不有趣，记录相应信息
            println!("Test failed or was not interesting.");
        }
    
        None
    }

    fn perform_calibrate_addition(
        &mut self,
        m1_m2_vec: &VecGraph,
        snapshot_state: &MutatorSnapshotState,
        offset: usize,
    ) -> Option<(TestInfo, usize, usize,usize,bool)> {
        // 0xff 测量
        if let Some((test_info, cf_index, vf_index,cfc_index,isstable)) = self.perform_run_get_testinfo(
            |def_mutator, rng, storage| {
                // 调用 mutate_data_full_bit_flip 进行变异操作
                def_mutator.mutate_data_addition(m1_m2_vec, snapshot_state, storage, rng, offset)
            },
        ) {
            // 打印测试结果和索引信息
            // println!("ADD Info: {:?}", test_info);
            // println!("CALIBRATE OFFSET:{}",offset);
            // println!("CF Index: {}", cf_index);
            // println!("VF Index: {}", vf_index);
            // 返回三元组
            return Some((test_info, cf_index, vf_index, cfc_index, isstable));
        } else {
            // 如果测试失败或不有趣，记录相应信息
            println!("Test failed or was not interesting.");
        }
    
        None
    }

    fn perform_calibrate_subtraction(
        &mut self,
        m1_m2_vec: &VecGraph,
        snapshot_state: &MutatorSnapshotState,
        offset: usize,
    ) -> Option<(TestInfo, usize, usize,usize,bool)> {
        // 0xff 测量
        if let Some((test_info, cf_index, vf_index,cfc_index,isstable)) = self.perform_run_get_testinfo(
            |def_mutator, rng, storage| {
                // 调用 mutate_data_full_bit_flip 进行变异操作
                def_mutator.mutate_data_subtraction(m1_m2_vec, snapshot_state, storage, rng, offset)
            },
        ) {
            // // 打印测试结果和索引信息
            // println!("SUB Info: {:?}", test_info);
            // println!("CALIBRATE OFFSET:{}",offset);
            // println!("CF Index: {}", cf_index);
            // println!("VF Index: {}", vf_index);
            // 返回三元组
            return Some((test_info, cf_index, vf_index, cfc_index, isstable));
        } else {
            // 如果测试失败或不有趣，记录相应信息
            println!("Test failed or was not interesting.");
        }
    
        None
    }

    fn save_results_to_json(results: &SequenceCalibrationResults, file_name: &str) -> std::io::Result<()> {
        let json_output = serde_json::to_string_pretty(results)?;
        let mut file = File::create(file_name)?;
        file.write_all(json_output.as_bytes())?;
        Ok(())
    }
    

    //测量队列所有测试用例
    pub fn calibrate_all_queue(&mut self) {
        if self.queue.len() == 0 {
            eprintln!("Queue is empty. No test cases to calibrate.");
            return;
        }
    
        for id in 0..self.queue.len() {
            if let Ok(entry) = self.queue.schedule(id).read() {
                let entry = entry.clone();
    
                let num_ops = std::cmp::min(
                    entry.ops_used as usize,
                    entry.data.node_len(&self.mutator.spec),
                );

                // 获取测试用例的数据
                // 打印
                let packet_data_bytes = entry.data.data_as_slice(); // 获取数据切片
                // 将字节数组转换为十六进制字符串
                let mut hex_encoded_data = String::new();
                for byte in packet_data_bytes {
                    fmt::write(
                        &mut hex_encoded_data,
                        format_args!("{:02x}", byte), // 使用 format_args 进行格式化
                    ).unwrap(); // 转换为两位的十六进制字符串
                }
                
                // println!("packet_data:{:?}",packet_data_bytes);
                let mut sequence_results = SequenceCalibrationResults {
                    sequence_id: id, // 假设队列中每一个测试用例是一个序列
                    cal_time: 0.0,
                    pkt_number: num_ops,
                    packets_cali_result: Vec::new(),
                    raw_data: Some(hex_encoded_data),
                };

                let start_time = self.queue.get_runtime_as_secs_f32();

                println!(
                    "[Analyzer] Calibrating test case {} with {} packets...",
                    id, num_ops
                );

                for snap_point in 0..num_ops {
                    print!("\r\x1B[Kpacket: {}/{}", snap_point+1, num_ops);  // \x1B[K 清除整行
                    io::stdout().flush().unwrap();
                    self.calibrate_with_snap(&entry, snap_point, &mut sequence_results,num_ops);
                    // self.calibrate_with_no_snap(&entry, snap_point, &mut sequence_results);
                }

                let end_time = self.queue.get_runtime_as_secs_f32();
                sequence_results.cal_time = end_time - start_time;
                let file_name = format!("calibration_results_sequence_{}.json", id);
                let output_path = std::path::Path::new(&self.config.workdir_path).join(file_name);
    
                if let Err(e) = Self::save_results_to_json(&sequence_results,  output_path.to_str().unwrap()) {
                    eprintln!("\n[Analyzer] Failed to save results for sequence {}: {}", id, e);
                } else {
                    println!("\n[Analyzer] Successfully saved results to {:?}", output_path);
                }
            } else {
                eprintln!("\n[Analyzer] Failed to read entry for id {}", id);
            }
        }
    }
    

    #[inline]
    fn calibrate_with_snap(
        &mut self, entry: &Input,
        snapshot_cutoff: usize, 
        sequence_results: &mut SequenceCalibrationResults,
        num_ops:usize,
    ) {
        let mut storage = self.fuzzer.get_struct_storage(self.mutator.spec.checksum);
        let mutator_state = self.mutator.prepare_snapshot(snapshot_cutoff, &entry.data, &mut storage, &self.rng);
        //create the snapshot
        // let payload = storage.as_vec_graph();
        // println!("input: {}",payload.to_script(&self.mutator.spec));
        // write!(&mut self.mutation_log, "MUTATE SNAPSHOT {:?} skipping first {:?} bytes\n",entry.data.data_as_slice(), mutator_state.skip_data);
        // println!("[SNAPSHOT INFO]:\n {}\n skipping first {:?} bytes\n",storage.as_vec_graph().to_script(&self.mutator.spec), mutator_state.skip_data);
        //qemu执行创建快照执行havoc测试
        if self.fuzzer.run_create_snapshot() {
            //获取snapshot_cutoff后一个包的数据：
            let mut m1_m2_vec = VecGraph::empty();
            let m1_m2_len = mutator_state.skip_nodes + 1;
            m1_m2_vec.copy_from_cutoff(&entry.data,m1_m2_len, &self.mutator.spec);
            let calibrate_len = m1_m2_vec.get_last_node_data_length(&self.mutator.spec);
            // let tested_packet = 
            // println!("START CALIBRATE");
            let standard =self.perform_calibrate_no_mutation(&m1_m2_vec, &mutator_state);
            if let Some((_, cf, vf,cfc,st)) = standard {
                let standard_packet = PacketCalibrationResult {
                    packet_id: snapshot_cutoff, // 当前包ID
                    offset: 0, // 标准结果不依赖偏移量
                    stable: st,
                    mutation_operator: "None".to_string(),    // 使用的变异算子
                    cf_index: cf,
                    vf_index: vf,
                    cfc_index: cfc,
                };
                sequence_results.packets_cali_result.push(standard_packet);
            } else {
                println!("Standard calibration failed or returned no result.");
            }            

            for offset in 0..calibrate_len {
                print!("\r\x1B[K packet:{}/{} offset: {}/{}",snapshot_cutoff+1 ,num_ops,offset, calibrate_len);  // \x1B[K 清除整行
                io::stdout().flush().unwrap();

                if let Some((_test_info, cf, vf,cfc,st)) =
                    self.perform_calibrate_lowest_bit_flip(&m1_m2_vec, &mutator_state, offset)
                    {
                        sequence_results.packets_cali_result.push(PacketCalibrationResult {
                            packet_id: snapshot_cutoff,
                            offset,
                            stable: st,
                            mutation_operator:"LBF".to_string(),
                            cf_index:cf,
                            vf_index:vf,
                            cfc_index: cfc,
                        });
                    }

                    if let Some((_test_info, cf, vf,cfc,st)) =
                self.perform_calibrate_full_bit_flip(&m1_m2_vec, &mutator_state, offset)
                    {
                        sequence_results.packets_cali_result.push(PacketCalibrationResult {
                            packet_id: snapshot_cutoff,
                            offset,
                            stable: st,
                            mutation_operator:"FBF".to_string(),
                            cf_index:cf,
                            vf_index:vf,
                            cfc_index: cfc,
                        });
                    }

                    if let Some((_test_info, cf, vf,cfc,st)) =
                self.perform_calibrate_addition(&m1_m2_vec, &mutator_state, offset)
                    {
                        sequence_results.packets_cali_result.push(PacketCalibrationResult {
                            packet_id: snapshot_cutoff,
                            offset,
                            stable: st,
                            mutation_operator:"ADD".to_string(),
                            cf_index:cf,
                            vf_index:vf,
                            cfc_index: cfc,
                        });
                    }

                    if let Some((_test_info, cf, vf,cfc,st)) =
                    self.perform_calibrate_subtraction(&m1_m2_vec, &mutator_state, offset)
                    {
                        sequence_results.packets_cali_result.push(PacketCalibrationResult {
                            packet_id: snapshot_cutoff,
                            offset,
                            stable: st,
                            mutation_operator:"SUB".to_string(),
                            cf_index:cf,
                            vf_index:vf,
                            cfc_index: cfc,
                        });
                    }
            }
            // println!("Calibration completed for pkt: {}", snapshot_cutoff);
            self.fuzzer.delete_snapshot().unwrap();
        }
    }

    // fn calibrate_with_no_snap(
    //     &mut self, entry: &Input,
    //     snapshot_cutoff: usize, 
    //     sequence_results: &mut SequenceCalibrationResults
    // ){        
    //         //获取snapshot_cutoff后一个包的数据：
    //         let mut storage = self.fuzzer.get_struct_storage(self.mutator.spec.checksum);
    //         let mut m1_m2_vec = VecGraph::empty();
    //         let m1_m2_len = snapshot_cutoff + 1;
    //         m1_m2_vec.copy_from_cutoff(&entry.data,m1_m2_len, &self.mutator.spec);
    //         let calibrate_len = m1_m2_vec.get_last_node_data_length(&self.mutator.spec);
    //         // let tested_packet = 
    //         // println!("START CALIBRATE");
    //         // let mutator_state = self.mutator.prepare_snapshot(snapshot_cutoff, &entry.data, &mut storage, &self.rng);
    //         self.mutator.prune_k(&entry.data, &mut storage, &self.rng, snapshot_cutoff);

    //         let standard =self.perform_calibrate_no_mutation(&m1_m2_vec, &MutatorSnapshotState::none());

    //         if let Some((_, cf, vf,cfc)) = standard {
    //             let standard_packet = PacketCalibrationResult {
    //                 packet_id: snapshot_cutoff, // 当前包ID
    //                 offset: 0, // 标准结果不依赖偏移量
    //                 mutation_operator: "None".to_string(),    // 使用的变异算子
    //                 cf_index: cf,
    //                 vf_index: vf,
    //                 cfc_index: cfc,
    //             };
    //             sequence_results.packets_cali_result.push(standard_packet);
    //         } else {
    //             println!("Standard calibration failed or returned no result.");
    //         }            

    //         for offset in 0..calibrate_len {
    //             println!("offset: {}/{}",offset,calibrate_len-1);
    //             if let Some((_test_info, cf, vf,cfc)) =
    //             self.perform_calibrate_lowest_bit_flip(&m1_m2_vec, &MutatorSnapshotState::none(), offset)
    //             {
    //                 sequence_results.packets_cali_result.push(PacketCalibrationResult {
    //                     packet_id: snapshot_cutoff,
    //                     offset,
    //                     mutation_operator:"LBF".to_string(),
    //                     cf_index:cf,
    //                     vf_index:vf,
    //                     cfc_index: cfc,
    //                 });
    //             }

    //             if let Some((_test_info, cf, vf,cfc)) =
    //             self.perform_calibrate_full_bit_flip(&m1_m2_vec, &MutatorSnapshotState::none(), offset)
    //             {
    //                 sequence_results.packets_cali_result.push(PacketCalibrationResult {
    //                     packet_id: snapshot_cutoff,
    //                     offset,
    //                     mutation_operator:"FBF".to_string(),
    //                     cf_index:cf,
    //                     vf_index:vf,
    //                     cfc_index: cfc,
    //                 });
    //             }

    //             if let Some((_test_info, cf, vf,cfc)) =
    //             self.perform_calibrate_addition(&m1_m2_vec, &MutatorSnapshotState::none(), offset)
    //             {
    //                 sequence_results.packets_cali_result.push(PacketCalibrationResult {
    //                     packet_id: snapshot_cutoff,
    //                     offset,
    //                     mutation_operator:"ADD".to_string(),
    //                     cf_index:cf,
    //                     vf_index:vf,
    //                     cfc_index: cfc,
    //                 });
    //             }

    //             if let Some((_test_info, cf, vf,cfc)) =
    //             self.perform_calibrate_subtraction(&m1_m2_vec, &MutatorSnapshotState::none(), offset)
    //             {
    //                 sequence_results.packets_cali_result.push(PacketCalibrationResult {
    //                     packet_id: snapshot_cutoff,
    //                     offset,
    //                     mutation_operator:"SUB".to_string(),
    //                     cf_index:cf,
    //                     vf_index:vf,
    //                     cfc_index: cfc,
    //                 });
    //             }
    //         }
        
    // }



    //开始
    pub fn run(&mut self) {
        use glob::glob;
        use std::time::Duration;
        //0号线程对应的fuzzer先导入测试用例：perform_import(true)
        if self.config.thread_id == 0 {
            self.perform_import(true);
        }
        else{
            while glob(&format!("{}/seeds/*.bin",self.config.workdir_path)).expect("Failed to read glob pattern").count() != 0{
                std::thread::sleep(Duration::from_millis(1000));
            }
        }
        /////测量开始
        self.calibrate_all_queue()
    
    }

    pub fn shutdown(&mut self) {
        self.fuzzer.shutdown().unwrap();
    }

}