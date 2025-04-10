use serde::Serialize;

use crate::bitmap::{BitmapHandler, StorageReason};
use crate::config::FuzzerConfig;
use crate::fuzz_runner::ExitReason;
// use crate::structured_fuzzer::custom_dict::CustomDict;
use crate::input::{Input, InputID};
use crate::structured_fuzzer::graph_mutator::graph_storage::{GraphStorage, VecGraph};
use crate::structured_fuzzer::mutator::InputQueue;
use crate::structured_fuzzer::random::distributions::Distributions;
use crate::structured_fuzzer::mutator::MutationStrategy;
use crate::structured_fuzzer::GraphSpec;
// use crate::snap_tree::SnapTree;
use std::collections::HashMap;

use std::sync::Arc;
use std::sync::RwLock;

#[derive(Serialize)]
pub struct QueueStats {
    num_inputs: usize,
    favqueue: Vec<usize>,
}

pub struct QueueData {
    bitmap_index_to_min_example: HashMap<usize, InputID>,
    //bitmap_index_to_max_example: HashMap<usize, InputID>,
    //ijon_index_to_max: HashMap<usize, InputID>,
    favqueue: Vec<InputID>,                         //最爱队列
    inputs: Vec<Arc<RwLock<Input>>>,
    // inputs_snap_tree: SnapTree,
    input_to_iters_no_finds: Vec<usize>,
    input_selected_times: Vec<usize>,
    bitmap_bits: Vec<usize>,                        //bitmap的bit
    bitmaps: BitmapHandler,
    next_input_id: usize,
}

#[derive(Clone)]
pub struct Queue {
    workdir: String,
    start_time: std::time::Instant,
    total_execs: Arc<RwLock<u64>>,
    data: Arc<RwLock<QueueData>>,           //队列智能指针
}

impl<'a> InputQueue for Queue {
    //从Queue中的inputs集合中随机选择一个元素，并返回其数据的克隆
    fn sample_for_splicing(&self, dist: &Distributions) -> Arc<VecGraph> {
        let data_lock = self.data.read().unwrap();
        let inputs = &data_lock.inputs;
        let i = dist.gen_range(0, inputs.len());
        let inp = inputs[i].read().unwrap().data.clone();
        return inp;
    }
}

impl Queue {
    ///新建模糊测试队列Queue，并初始化。
    /// 
    /// 根据FuzzerConfig配置信息来设置工作目录、位图处理器的大小。
    pub fn new(config: &FuzzerConfig) -> Self {
        return Self {
            //队列的基本属性
            workdir: config.workdir_path.clone(),
            start_time: std::time::Instant::now(),
            total_execs: Arc::new(RwLock::new(0_u64)),
            //队列记录的测试用例
            data: Arc::new(RwLock::new(QueueData {
                bitmap_index_to_min_example: HashMap::new(),
                //bitmap_index_to_max_example: HashMap::new(),
                //ijon_index_to_max: HashMap::new(),
                inputs: vec![],
                // inputs_snap_tree:SnapTree::new(),
                favqueue: vec![],
                input_to_iters_no_finds: vec![],
                input_selected_times:vec![],
                bitmap_bits: vec![],
                bitmaps: BitmapHandler::new(config.bitmap_size),
                next_input_id: 0,
            })),
        };
    }

    // pub fn update_total_execs(&self, update: u64){
    //     let mut w = self.total_execs.write().unwrap();
    //     *w += update; 
    // }

    pub fn get_total_execs(&self) -> u64 {
        *self.total_execs.read().unwrap()
    }

    pub fn get_runtime_as_secs_f32(&self) -> f32 {
        (std::time::Instant::now() - self.start_time).as_secs_f32()
    }

    pub fn write_stats(&self) {
        use std::fs::File;
        use std::fs::OpenOptions;
        use std::io::prelude::*;
        //读取数据
        let dat = self.data.read().unwrap();
        let ser = QueueStats {
            num_inputs: dat.inputs.len(),
            favqueue: dat
                .favqueue
                .iter()
                .map(|id| id.as_usize())
                .collect::<Vec<_>>(),
        };
        //写入队列统计信息到queue_stats.msgp
        let mut file = File::create(format!("{}/queue_stats.msgp", &self.workdir)).unwrap();
        rmp_serde::encode::write_named(&mut file, &ser).unwrap();
        //写入位图统计信息到bitmap_stats.txt
        let mut file = OpenOptions::new()
            .append(true)
            .create(true)
            .open(format!("{}/bitmap_stats.txt", &self.workdir))
            .unwrap();
        file.write_fmt(format_args!(
            "{},{}\n",
            (std::time::Instant::now() - self.start_time).as_secs_f32(),    //从Queue的start_time到当前时间的秒数
            dat.bitmaps.normal_bitmap().bits().iter().filter(|b| **b > 0).count()   //获取bitmaps中的normal_bitmap的hit数量
        ))
        .unwrap();
    }

    // pub fn num_bits(&self) -> usize {   //获取bitmaps中的normal_bitmap的hit数量
    //     return self.data.read().unwrap().bitmaps.normal_bitmap().bits().iter().filter(|b| **b > 0).count()
    // }

    pub fn len(&self) -> usize {    //获取输入队列中测试用例的数量
        return self.data.read().unwrap().inputs.len();
    }

    // pub fn num_crashes(&self) -> usize{     //获取输入队列中导致crash的测试用例的数量
    //     self.data.read().unwrap().inputs.iter().filter(|i| matches!(i.read().unwrap().exit_reason, ExitReason::Crash(_)) ).count()
    // }

    /// 检查程序执行当前测试用例的覆盖率信息，返回存储原因StorageReason，
    /// 
    /// 输入：当前运行生成的bitmap、运行的执行结果、使用的变异策略
    /// 
    /// 返回：存储原因StorageReason
    /// 
    /// 对于MutationStrategy是SeedImport，且ExitReason不是Timeout的测试用例，确定其StorageReason是Imported
    /// 
    pub fn check_new_bytes(
        &mut self,
        run_bitmap: &[u8],
        etype: &ExitReason,
        strat: MutationStrategy
    ) -> Option<Vec<StorageReason>> {
        
        // 根据bitmap信息，获取其StorageReason：res
        // 尝试获取`data`字段的写入锁定，并调用`bitmaps`字段的`check_new_bytes`方法。
        let mut res = self.data 
            .write()
            .unwrap()
            .bitmaps
            .check_new_bytes(run_bitmap, etype);
         
        // 如果变异策略是SeedImport，并且退出原因不是超时，
        // 那么将`Imported`作为存储原因添加到结果中
        if strat == MutationStrategy::SeedImport && *etype != ExitReason::Timeout{
            let mut reasons = res.take().unwrap_or(vec!());
            reasons.push(StorageReason::Imported);
            res = Some(reasons);
        }

        return res;
    }

    /// 注册bitmap对应的最佳输入。
    /// 
    /// 类似AFL为bitmap选择最短序列的功能
    /// 
    pub fn register_best_input_for_bitmap(data: &mut std::sync::RwLockWriteGuard<QueueData>, bitmap_index: usize, input_id: InputID, spec: &GraphSpec, new_len: usize){
        //检查位图索引：
        //检查 data.bitmap_index_to_min_example 是否已经包含了 bitmap_index。
        //如果没有，这意味着这是该位图索引的第一个输入，
            //将 bitmap_index 添加到 data.bitmap_bits 中。
            //在 data.bitmap_index_to_min_example 中插入 bitmap_index 和 input_id 的键值对。
        if !data.bitmap_index_to_min_example.contains_key(&bitmap_index) {
            data.bitmap_bits.push(bitmap_index);
            data.bitmap_index_to_min_example.insert(bitmap_index, input_id);
        }

        //获取旧条目：
        let old_entry = data
            .bitmap_index_to_min_example
            .get_mut(&bitmap_index)
            .unwrap()
            .as_usize();

        //比较节点长度：如果旧输入ID对应的数据节点长度大于 new_len，则说明新输入更优（因为长度更短），函数会在 data.bitmap_index_to_min_example 中更新 bitmap_index 对应的输入ID为新的 input_id
        if data.inputs[old_entry].read().unwrap().data.node_len(&spec) > new_len {
            data.bitmap_index_to_min_example.insert(bitmap_index, input_id);
        }
    }

    /// 注册ijon bitmap对应的最佳输入。
    // pub fn register_best_input_for_ijon_max(data: &mut std::sync::RwLockWriteGuard<QueueData>, ijon_index: usize, input_id: InputID){
    //     data.bitmap_index_to_min_example.insert(ijon_index, input_id);
    // }

    /// 向内存队列中添加一个新的input，并更新用bitmap触发情况管理的测试用例
    /// 
    /// 输出：保存的新input的分配的id
    /// 
    pub fn add(&mut self, mut input: Input, spec: &GraphSpec) -> Option<InputID> {
        assert_eq!(input.id, InputID::invalid());
        if input.data.node_len(spec) == 0 {
            return None;
        }
        let id;
         // 根据测试用例的退出原因进行匹配。
        match input.exit_reason {
             // 如果退出原因是正常或崩溃。
            ExitReason::Normal(_) | ExitReason::Crash(_) | ExitReason::Timeout => {
                //let has_new_bytes = input.storage_reasons.iter().any(|r| r.has_new_byte() );
                //let should_update_favs;
                {
                    let mut data = self.data.write().unwrap();
                    //should_update_favs = has_new_bytes;
                    id = InputID::new(data.inputs.len()); // 为输入生成一个新的ID。
                    input.id = id;
                    let new_len = input.data.node_len(&spec);
                    let input = Arc::new(RwLock::new(input));
                    data.inputs.push(input.clone());
                    data.input_to_iters_no_finds.push(0);
                    data.input_selected_times.push(0);
                    // match input.read().unwrap().found_by{
                    //     MutationStrategy::SeedImport =>
                    //     {
                    //         let input_id = input.read().unwrap().id;
                    //         // println!("add_node_to_root:{}",input_id.as_usize());
                    //         data.inputs_snap_tree.add_node_to_root(input_id)
                    //     }
                    //     _=>
                    //     {
                    //         let input_id = input.read().unwrap().id;
                    //         let patent_id = input.read().unwrap().parent_id;
                    //         // println!("add_node:{}->{}",patent_id.as_usize(),input_id.as_usize());
                    //         data.inputs_snap_tree.add_node(input_id,patent_id)
                    //     }
                    // }

                     // 遍历输入的存储原因，并根据原因类型进行处理。
                    for r in input.read().unwrap().storage_reasons.iter() {
                        match r {
                            //如果是bitmap hit的原因，注册最佳输入
                            StorageReason::Bitmap(reason) => Self::register_best_input_for_bitmap(&mut data, reason.index, id, spec, new_len),
                            // 如果是Ijonmax原因，注册ijonmax最佳输入。
                            // StorageReason::IjonMax(reason) => Self::register_best_input_for_ijon_max(&mut data,reason.index, id),
                            // 如果是导入原因，不做处理。
                            StorageReason::Imported => {},
                        }
                    }
                }
                // 更新fav的位。
                //if should_update_favs {
                    self.calc_fav_bits();
                //}
                // 写入统计数据。
                self.write_stats();
            }
            /*
            ExitReason::Crash(_) => return None, //println!("NEW crash found!"),
            */
            // 如果退出原因是其他，不添加输入，并返回None。
            _ => {
                //println!("ignoring input {:?}", input.exit_reason);
                return None;
            }
        }
        // 返回新生成的输入ID。
        return Some(id);
    }

    //计算和更新模糊测试中被认为是有价值的输入的集合
    pub fn calc_fav_bits(&mut self) {
        let mut favids = vec![];
        // let mut ijon_slot_to_fav = HashMap::<usize,(u64,InputID)>::new();
        //println!("==== update favbit queue store ====");
        {
            //const IJON_MAX_SIZE: usize = 256;
            let data = self.data.read().unwrap();
            let mut bits = vec![0u8; data.bitmaps.size()]; // 初始化一个位图大小的向量，用于标记已经发现的新位。

            // 遍历输入集合。
            for input in data.inputs.iter().rev() {
                let inp = input.read().unwrap();
                // 如果输入有新的字节。
                if inp.storage_reasons.iter().any(|s| s.has_new_byte() ) {
                    //found new bytes
                     // 检查是否有新的位被设置。
                    let has_new_bits = inp
                    .bitmap
                    .bits()
                    .iter()
                    .enumerate()
                    .any(|(i, v)| bits[i] == 0 && *v > 0);
                // 如果有新的bit被设置，更新bitmap并将这个测试用例添加到喜爱的ID集合中。
                    if  has_new_bits {
                        for (i, v) in inp.bitmap.bits().iter().enumerate() {
                            if *v != 0 {
                                bits[i] = 1;
                            }
                        }
                        favids.push(inp.id);
                    }
                    // 更新ijon最大值槽位的喜爱输入。
                    // for (i, v) in inp.bitmap.ijon_max_vals().iter().enumerate() {
                    //     if *v != 0 && (!ijon_slot_to_fav.contains_key(&i) ||  ijon_slot_to_fav[&i].0<*v) {
                    //         ijon_slot_to_fav.insert(i,(*v,inp.id));
                    //     }
                    // }
                     // 如果输入是通过种子导入找到的，也添加到喜爱的ID集合中。
                } else if inp.found_by == MutationStrategy::SeedImport{
                    favids.push(inp.id);
                }
            }
        }

         // 遍历ijon槽位的喜爱输入。
        // for (i,(_val,id)) in ijon_slot_to_fav.iter(){
        //      // 如果喜爱的ID集合中还没有这个ID，添加进去。
        //     //use std::time::SystemTime;
        //     println!("[!] store ijon {:?} for {} => {:x}",
        //         //SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_secs(),
        //         id,i,_val);
        //     if !favids.contains(id) { //TODO FIX O(n)
        //         favids.push(*id); 
        //     }
        // }
        {
            let mut data = self.data.write().unwrap();
            /* 
            println!(
                "calc favbits ({}) out of {}",
                favids.len(),
                data.inputs.len()
            );
            */
            //更新favqueue
            data.favqueue = favids;
        }
    }

    //直接根据指定的id拿input
    pub fn schedule(&self, id:usize) -> Arc<RwLock<Input>> {
        self.data.read().unwrap().inputs[id].clone()
    }

    //用于生成下一个输入的唯一标识符（ID）
    pub fn next_id(&mut self) -> usize {
        let mut data = self.data.write().unwrap();
        data.next_input_id += 1;
        return data.next_input_id;
    }


}
