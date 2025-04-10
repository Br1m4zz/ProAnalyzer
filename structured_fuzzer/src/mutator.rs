use std::rc::Rc;
//use std::borrow::Borrow;
use std::sync::Arc;

use crate::graph_mutator::graph_builder::{GraphBuilder,GraphState};
use crate::graph_mutator::graph_iter::GraphNode;
use crate::graph_mutator::graph_storage::GraphStorage;
use crate::graph_mutator::graph_storage::VecGraph;
use crate::graph_mutator::spec::GraphSpec;
use crate::primitive_mutator::mutator::{PrimitiveMutator, PrimitiveMutatorDefenite};
use crate::random::distributions::Distributions;
use crate::custom_dict::CustomDict;

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum MutationStrategy{
    GenerateTail(GenerateTail),
    SpliceRandom,
    Splice,
    DataOnly,
    Generate,
    Repeat,
    Minimize,
    MinimizeSplit,
    Import,
    SeedImport,
}

impl MutationStrategy{
    pub fn name(&self) -> &str{
        match self{
            MutationStrategy::GenerateTail(_) => "generate_tail",
            MutationStrategy::SpliceRandom => "splice_random",
            MutationStrategy::Splice => "splice",
            MutationStrategy::DataOnly => "data_only",
            MutationStrategy::Generate => "generate",
            MutationStrategy::Repeat => "repeat",
            MutationStrategy::Minimize => "minimize",
            MutationStrategy::MinimizeSplit => "minimize_split",
            MutationStrategy::Import=>"import",
            MutationStrategy::SeedImport=>"seed_import"
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub struct GenerateTail{ pub drop_last: usize, pub generate: usize }//记录上次

#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
pub enum NodeMutationType {
    CopyNode,
    MutateNodeData,
    DropNode,
    SkipAndGenerate,
}

/// 记录测试用例拍摄增量快照的前缀消息信息，
/// 
/// 对应跳过的操作节点、操作数据信息
/// 
/// fuzzer会根据这个信息摘出后续的操作
/// 
pub struct MutatorSnapshotState{
    pub skip_nodes: usize,//拍摄快照跳过的节点
    pub skip_ops: usize,//跳过的操作
    pub skip_data: usize,//跳过的数据
    pub prefix_graph_state: Option<GraphState>, // 拍摄快照会记录跳过的前缀对应的specgraph状态
}

impl MutatorSnapshotState{
    pub fn none() -> Self {
        return Self{skip_data:0, skip_nodes: 0, skip_ops:0, prefix_graph_state: None}
    }
}

/// specfuzzer的spec变异器。
/// 变异围绕着测试用例的vecgraph展开
/// 
/// spec是被测对象使用的spec
/// builder管理当前的spec图
/// mutator是数据变异器
/// 
pub struct Mutator {
    pub spec: Rc<GraphSpec>,
    builder: GraphBuilder,
    mutator: PrimitiveMutator,
}

pub trait InputQueue {
    fn sample_for_splicing(&self, dist: &Distributions) -> Arc<VecGraph>;
}

impl InputQueue for Vec<VecGraph>{
    fn sample_for_splicing(&self, dist: &Distributions) -> Arc<VecGraph>{
        assert!(self.len() > 0);
        return Arc::new(self[dist.gen_range(0,self.len())].clone());
    }
}

impl Mutator {
    /// spec变异器构造函数。基于传入的spec构建对应的变异器
    /// 
    /// specfuzz的输入模式的变异灵魂
    pub fn new(spec: GraphSpec) -> Self {
        let spec = Rc::new(spec);                       //导入spec，并转化为引用计数智能指针，方便后续引用spec
        let mutator = PrimitiveMutator::new();          //新建一个PrimitiveMutator
        let builder = GraphBuilder::new(spec.clone());  //根据传入的spec，构建specgraph

        return Self {
            spec,
            builder,
            mutator,
        };
    }

/// 对传入的orig输入对应的VecGraph进行变异，
/// 
/// 输入：测试用例中ops的数量ops_used，测试用例适用的字典dict。该测试用例快照拍摄的前缀信息snapshot，模糊测试的队列queue。
/// 
/// 输出：具体使用的havoc变异策略
    pub fn mutate<S: GraphStorage, Q: InputQueue>(&mut self, orig: &VecGraph, ops_used: usize, dict: &CustomDict, snapshot: &MutatorSnapshotState, queue: &Q, storage: &mut S, dist: &Distributions) -> MutationStrategy{

        //获取快照点后缀的payload的长度
        let orig_len =  ops_used as usize-snapshot.skip_nodes;

        if orig.op_len()== 0 || orig_len == 0 || ops_used == 0 {
            self.generate(50, snapshot, storage, dist);
            return MutationStrategy::Generate;
        }//没有初始种子，则使用生成的方法

        let strategy = dist.gen_mutation_strategy(orig_len);
        match strategy{//根据策略执行变异
            MutationStrategy::GenerateTail(args) => self.generate_tail(orig, ops_used, snapshot, args, storage, dist),
            MutationStrategy::SpliceRandom => self.splice_random(orig, ops_used, snapshot, dict, storage, dist),
            MutationStrategy::Splice => self.splice(orig, ops_used, snapshot,queue, storage, dist),
            MutationStrategy::DataOnly => self.mutate_data(orig, ops_used, snapshot, dict, storage, dist),
            MutationStrategy::Generate => self.generate(50, snapshot, storage, dist),
            MutationStrategy::Repeat => self.repeat(orig, ops_used, snapshot, dict, storage, dist),
            MutationStrategy::Minimize => unreachable!(),
            MutationStrategy::MinimizeSplit => unreachable!(),
            MutationStrategy::Import => unreachable!(),
            MutationStrategy::SeedImport => unreachable!(),
        }
        return strategy;//返回具体选择的策略
    }

    /// 根据传入的测试用例data准备快照。
    /// 
    /// 使用builder为storage新建一个空输入, 并根据snapshot_cutoff将输入中的操作划分为前缀和尾缀，
    /// 
    /// 把data中的前缀增添到builder中，记录对应的信息后，再添加快照操作节点。
    /// 
    /// 返回测试用例快照前缀的状态信息：MutatorSnapshotState。这个信息用于恢复测试用例的变异信息
    pub fn prepare_snapshot<S: GraphStorage>(&mut self, snapshot_cutoff: usize, data: &VecGraph, storage: &mut S, dist: &Distributions) -> MutatorSnapshotState{
        //先清空当前的graphbuilder
        self.builder.start(storage, &MutatorSnapshotState::none());
        //取测试用例内容的VecGraph中取前snapshot_cutoff个节点，graphbuilder会在末尾添加n指向的snapshot_cutoff位置的节点
        for n in data.node_iter(&self.spec).take(snapshot_cutoff){
            self.builder.append_node(&n, storage, dist);
        }

        //更新graphbuilder记录了前缀的的prefix_graph_state
        let prefix_graph_state = Some(self.builder.get_graph_state());
        let skip_ops =storage.op_len();
        let skip_data = storage.data_len();
        //对应的添加snapshot节点
        storage.append_op(self.spec.snapshot_node_id.unwrap().as_u16());
        //返回记录的对应的MutatorSnapshotState
        return MutatorSnapshotState{skip_nodes: snapshot_cutoff, skip_ops, skip_data, prefix_graph_state};
    }

    ///重复变异
    pub fn repeat<S: GraphStorage>(&mut self, orig: &VecGraph, ops_used: usize, snapshot: &MutatorSnapshotState, dict: &CustomDict, storage: &mut S, dist: &Distributions) {
        self.builder.start(storage, snapshot);

        let fragment_nodes = dist.gen_range(2, 16);
        let repeats = dist.gen_range(2, 6);
        let insert_pos = if ops_used-snapshot.skip_nodes-1 > 0 {
         dist.gen_range(0, ops_used-snapshot.skip_nodes-1)
        } else { 0 };
        //println!("REPEAT {}..{} (out of {} with {} skipped) for {} times",insert_pos, insert_pos+fragment_nodes, orig.node_len(&self.spec), snapshot.skip_nodes, repeats);
        for n in orig.node_iter(&self.spec.clone()).skip(snapshot.skip_nodes).take(insert_pos) {
            if self.builder.is_full(storage){return;}
            self.builder.append_node(&n, storage, dist); 
        }

        let origs = orig.node_iter(&self.spec).skip(snapshot.skip_nodes+insert_pos).take(fragment_nodes).collect::<Vec<_>>();
        assert!(origs.len() > 0);
        assert!(repeats > 1);
        for _ in 0..repeats {
            for n in origs.iter() {
                if self.builder.is_full(storage){return;}
                self.builder.append_node_mutated(&n, dict, &self.mutator, storage, dist);
            }
        }

        for n in orig.node_iter(&self.spec.clone()).skip(snapshot.skip_nodes+insert_pos) {
            if self.builder.is_full(storage){return;}
            self.builder.append_node(&n, storage, dist); 
        }
    }

    /// 在尾部做生成的变异，
    /// 
    /// (pre snaped) + post + append_random[new]
    /// 
    /// 输入：原始输入的数据orig、原始输入的操作数量ops_used、输入的快照记录信息snapshot、尾部生成策略的参数args、SPEC的storage、变异的随机分布dist
    /// 
    /// 
    pub fn generate_tail<S: GraphStorage>(&mut self,  orig: &VecGraph, ops_used: usize, snapshot: &MutatorSnapshotState, args: GenerateTail, storage: &mut S, dist: &Distributions){
        let orig_len =  ops_used-snapshot.skip_nodes;
        //根据snapshot记录的信息，恢复对应的builder
        self.builder.start(storage, snapshot); 

        //跳过快照点前面的操作后，跳过orig_len-drop_last个节点(post)，添加新的操作节点
        for n in orig.node_iter(&self.spec).skip(snapshot.skip_nodes).take(orig_len-args.drop_last) {
            if self.builder.is_full(storage){return;}//如果builder记录满了就不操作
            self.builder.append_node(&n, storage, dist);//在末尾添加操作节点
        }
        
        //根据generate的大小，随机添加节点
        self.builder.append_random(args.generate, &self.mutator, storage, dist).unwrap();
    }

    /// 数据内容变异，拿到一个vecGraph,跳过快照拍摄的哪些节点后，
    /// 
    /// (pre snaped) + append_node_data_mutated(post) 
    /// 
    pub fn mutate_data<S: GraphStorage>(&mut self, orig: &VecGraph, ops_used: usize, snapshot: &MutatorSnapshotState, dict: &CustomDict,  storage: &mut S, dist: &Distributions) {
        self.builder.start(storage, snapshot);
        for n in orig.node_iter(&self.spec.clone()).skip(snapshot.skip_nodes) {
            if self.builder.is_full(storage){return;}
            if dist.should_mutate_data(ops_used-snapshot.skip_nodes){ //TODO fix this with probability based on length
                self.builder.append_node_mutated(&n, dict, &self.mutator,  storage, dist);
            } else {//若判断不应该变异数据，那么就选择添加一个操作节点
                self.builder.append_node(&n, storage, dist); 
            }
        }
    }

    /// 随机拼接变异
    /// 
    /// (pre snaped) + append_node_with_spliced(post) 
    /// 
    pub fn splice_random<S: GraphStorage>(&mut self, orig: &VecGraph, ops_used: usize,  snapshot: &MutatorSnapshotState, dict: &CustomDict, storage: &mut S, dist: &Distributions) {
        self.builder.start(storage, snapshot);
        for n in orig.node_iter(&self.spec.clone()).skip(snapshot.skip_nodes).take(ops_used) {
            if self.builder.is_full(storage){return;}
            let mutation = self.pick_op(&n, dist);
            self.apply_graph_node(mutation, &n, dict, storage, dist);
        }
    }

    pub fn pick_splice_points(&self, len: usize, dist: &Distributions) -> Vec<usize>{
        use std::cmp::Reverse;
        let num = match len{
            0 => unreachable!(),
            1..=3 => dist.gen_range(1,3),
            4..=15 => dist.gen_range(1,5),
            _ => dist.gen_range(4,16),
        };
        let mut res = (0..num).map(|_| dist.gen_range(0,len) ).collect::<Vec<_>>();
        res.sort_unstable();
        res.sort_by_key(|x| (*x, Reverse(*x))); 
        res.dedup();
        return res;
    }

    /// 拼接变异
    pub fn splice< S: GraphStorage, Q:InputQueue >(&mut self, orig: &VecGraph, ops_used: usize, snapshot: &MutatorSnapshotState, queue: &Q, storage: &mut S, dist: &Distributions) {

        let orig_len = ops_used-snapshot.skip_nodes;
        let mut splice_points = self.pick_splice_points(orig_len, dist);
        //println!("splice with {:?} on graph with {}..{}/{}", splice_points, snapshot.skip_nodes, ops_used,orig.node_len(&self.spec));
        self.builder.start(storage, snapshot);
        let mut spliced = false;
        for (i,n) in orig.node_iter(&self.spec.clone()).skip(snapshot.skip_nodes).enumerate() {
            if self.builder.is_full(storage){return;}
            // if splice_points.len()>0{
            //     println!("{} vs {}",i,*splice_points.last().unwrap());
            // }
            // else {
            //     println!("err");
            // }
            if splice_points.len() > 0 && i == *splice_points.last().unwrap(){
                splice_points.pop();
                let other_lock = queue.sample_for_splicing(&dist);
                let other = other_lock.as_ref();
                let other_len = other.node_len(&self.spec);
                let start = dist.gen_range(0,other_len);
                let mut len = dist.gen_range(1,16);
                if len > other_len-start { len = other_len-start }
                assert!(len>0);
                for nn in other.node_iter(&self.spec.clone()).skip(start).take(len){
                    self.builder.append_node(&nn, storage, dist);
                }
                spliced=true;
            }
            self.builder.append_node(&n, storage, dist);
        }
        assert!(spliced);
    }

    /// 这个函数不会恢复快照时期的storage，而是直接在原来storage的基础上添加一串input
    pub fn append_input<S: GraphStorage>(&mut self, m3_orig: &VecGraph, storage: &mut S, dist: &Distributions) {
        //直接把M3_orig的所有数据节点附加到storage里
        for n in m3_orig.node_iter(&self.spec.clone()){
            self.builder.append_node(&n, storage, dist); 
        }
    }

    /// 将orig指定的VecGraph中的节点拷贝到GraphStorage之中
    /// 
    pub fn copy_all<S: GraphStorage>(&mut self, orig: &VecGraph,  storage: &mut S, dist: &Distributions){
        //由于来自测试用例，所以没有快照信息
        self.builder.start(storage, &MutatorSnapshotState::none());

        //对orig记录的每一个操作节点，根据当前refgraph情况，添加到storage之中
        for n in orig.node_iter(&self.spec){
            self.builder.append_node(&n, storage, dist);
        }
    }

    //截断前K个节点的输入
    pub fn prune_k<S: GraphStorage>(&mut self, orig: &VecGraph, storage: &mut S, dist: &Distributions, node_k: usize) {
        self.builder.start(storage, &MutatorSnapshotState::none());
        assert!(node_k <= orig.node_len(&self.spec), "node_k must be less than the length of orig");
        // 使用collect::<Vec<_>>()来收集需要交换的节点
        let nodes = orig.node_iter(&self.spec).collect::<Vec<_>>();
        // 确保node_k和node_k+1在范围内
        for i in 0..nodes.len() {
            if self.builder.is_full(storage) { return; }
            if i <= node_k{
                self.builder.append_node(&nodes[i], storage, dist);
            } else {
                break;
            }
        }
    }

    // pub fn copy_node<S: GraphStorage>(&mut self, node: GraphNode, storage: &mut S, dist: &Distributions) {
    //     self.builder.append_node(&node, storage, dist); 
    // }

    ///截断前K-1个节点的输入,对第K个输入的第M字节进行确定变异
    // pub fn append_k_fill_m<S: GraphStorage>(&mut self, orig: &VecGraph, storage: &mut S, dist: &Distributions, node_k: usize, byte_m: usize) {
    //     self.builder.start(storage, &MutatorSnapshotState::none());
    //     assert!(node_k <= orig.node_len(&self.spec), "node_k must be less than the length of orig");
    //     // 使用collect::<Vec<_>>()来收集需要交换的节点
    //     let nodes = orig.node_iter(&self.spec).collect::<Vec<_>>();
    //     // 确保node_k和node_k+1在范围内
    //     for i in 0..nodes.len() {
    //         if self.builder.is_full(storage) { return; }
            
    //         if i < node_k{
    //             self.builder.append_node(&nodes[i], storage, dist);
    //         } 
    //         else if i == node_k {
    //             self.builder.append_node_mutated_flip_at(&nodes[i], &self.mutator,storage, dist);
    //         }
    //     }
    // }

    ///生成变异
    pub fn generate<S: GraphStorage>(&mut self, n: usize, snapshot: &MutatorSnapshotState,  storage: &mut S, dist: &Distributions) {
        self.builder.start(storage, snapshot);
        self.builder
            .append_random(n, &self.mutator, storage, dist)
            .unwrap();
    }

    pub fn drop_range<S: GraphStorage>(
        &mut self,
        orig: &VecGraph,
        range: std::ops::Range<usize>,
        storage: &mut S,
        dist: &Distributions
    ) {
        self.builder.start(storage, &MutatorSnapshotState::none());
        for (i, n) in orig.node_iter(&self.spec.clone()).enumerate() {
            if range.start <= i && i < range.end {
                continue;
            }
            self.builder.append_node(&n, storage, dist);
        }
    }

    //pub fn drop_node_at<S: GraphStorage>(&mut self, orig: &VecGraph, i: usize, storage: &mut S, dist: &Distributions) {
    //    self.drop_range(orig, i..i + 1, storage, dist);
    //}

    pub fn dump_graph<S: GraphStorage>(&self, storage: &S) -> VecGraph {
        storage.as_vec_graph()
    }

    fn apply_graph_node<S: GraphStorage>(
        &mut self,
        op: NodeMutationType,
        n: &GraphNode,
        dict: &CustomDict,
        storage: &mut S,
        dist: &Distributions
    ) {
        use NodeMutationType::*;

        match op {
            CopyNode => {
                self.builder.append_node(&n, storage, dist);
            }
            MutateNodeData => self.builder.append_node_mutated(&n, dict, &self.mutator, storage, dist),
            DropNode => {}
            SkipAndGenerate => {
                let len = dist.gen_number_of_random_nodes();
                self.builder
                    .append_random(len, &self.mutator, storage, dist)
                    .unwrap();
            }
        }
    }

    fn pick_op(&self, _n: &GraphNode, dist: &Distributions) -> NodeMutationType {
        return dist.gen_graph_mutation_type();
    }

    pub fn num_ops_used<S: GraphStorage>(&self, storage: &S) -> usize {
        return self.builder.num_ops_used(storage);
    }
}

// #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
// pub struct FullBitFlip{ pub off: usize }
// #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
// pub struct LowBitFlip{ pub off: usize }
// #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
// pub struct BitAdd{ pub off: usize }
// #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
// pub struct BitSub{ pub off: usize }
// #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
// pub struct BitLen{ pub off: usize }
// #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug)]
// pub struct BitCheck{ pub off: usize }
// pub enum DetMutationStrategy{
//     FullBitFlip(FullBitFlip),
//     LowBitFlip(LowBitFlip),
//     BitAdd(BitAdd),
//     BitSub(BitSub),
//     BitLen(BitLen),
//     BitCheck(BitCheck)
// }

// impl DetMutationStrategy{
//     pub fn name(&self) -> &str{
//         match self{
//             DetMutationStrategy::FullBitFlip(_) => "FullBitFlip",
//             DetMutationStrategy::LowBitFlip(_) => "LowBitFlip",
//             DetMutationStrategy::BitAdd(_) => "BitAdd",
//             DetMutationStrategy::BitSub(_) => "BitSub",
//             DetMutationStrategy::BitLen (_)=> "BitLen",
//             DetMutationStrategy::BitCheck(_) => "BitCheck",
//         }
//     }
// }

pub struct DetMutator{
    pub spec: Rc<GraphSpec>,
    builder: GraphBuilder,
    mutator: PrimitiveMutatorDefenite,
}

impl DetMutator{
    pub fn new(spec: GraphSpec) -> Self {
        let spec = Rc::new(spec);                       //导入spec，并转化为引用计数智能指针，方便后续引用spec
        let mutator = PrimitiveMutatorDefenite::new();          //新建一个PrimitiveMutator
        let builder = GraphBuilder::new(spec.clone());  //根据传入的spec，构建specgraph
        return Self {
            spec,
            builder,
            mutator,
        };
    }

    pub fn copy_all<S: GraphStorage>(&mut self, orig: &VecGraph,  storage: &mut S, dist: &Distributions){
        //由于来自测试用例，所以没有快照信息
        self.builder.start(storage, &MutatorSnapshotState::none());

        //对orig记录的每一个操作节点，根据当前refgraph情况，添加到storage之中
        for n in orig.node_iter(&self.spec){
            self.builder.append_node(&n, storage, dist);
        }
    }

    /// 根据传入的测试用例data准备快照。
    pub fn prepare_snapshot<S: GraphStorage>(&mut self, snapshot_cutoff: usize, data: &VecGraph, storage: &mut S, dist: &Distributions) -> MutatorSnapshotState{
        //先清空当前的graphbuilder
        self.builder.start(storage, &MutatorSnapshotState::none());
        //取测试用例内容的VecGraph中取前snapshot_cutoff个节点，graphbuilder会在末尾添加n指向的snapshot_cutoff位置的节点
        for n in data.node_iter(&self.spec).take(snapshot_cutoff){
            self.builder.append_node(&n, storage, dist);
        }

        //更新graphbuilder记录了前缀的的prefix_graph_state
        let prefix_graph_state = Some(self.builder.get_graph_state());
        let skip_ops =storage.op_len();
        let skip_data = storage.data_len();
        //对应的添加snapshot节点
        storage.append_op(self.spec.snapshot_node_id.unwrap().as_u16());
        //返回记录的对应的MutatorSnapshotState
        return MutatorSnapshotState{skip_nodes: snapshot_cutoff, skip_ops, skip_data, prefix_graph_state};
    }

    ///截断前K个节点的输入
    pub fn prune_k<S: GraphStorage>(&mut self, orig: &VecGraph, storage: &mut S, dist: &Distributions, node_k: usize) {
        self.builder.start(storage, &MutatorSnapshotState::none());
        assert!(node_k <= orig.node_len(&self.spec), "node_k must be less than the length of orig");
        // 使用collect::<Vec<_>>()来收集需要交换的节点
        let nodes = orig.node_iter(&self.spec).collect::<Vec<_>>();
        // 确保node_k和node_k+1在范围内
        for i in 0..nodes.len() {
            if self.builder.is_full(storage) { return; }
            if i <= node_k{
                self.builder.append_node(&nodes[i], storage, dist);
            } else {
                break;
            }
        }
    }
    
    /// 全比特翻转变异：对节点数据内 off 位置处的字节进行全比特取反（^ 0xff）。
    pub fn append_unmutate<S: GraphStorage>(
        &mut self,
        orig: &VecGraph,
        snapshot: &MutatorSnapshotState,
        storage: &mut S,
        dist: &Distributions,
    ) {
        self.builder.start(storage, snapshot);
        if let Some(n) = orig.node_iter(&self.spec.clone()).skip(snapshot.skip_nodes).next() {
            if !self.builder.is_full(storage) {
                self.builder
                    .append_node(&n, storage, dist);
            }
        }
    }

    /// 全比特翻转变异：对节点数据内 off 位置处的字节进行全比特取反（^ 0xff）。
    pub fn mutate_data_full_bit_flip<S: GraphStorage>(
        &mut self,
        orig: &VecGraph,
        snapshot: &MutatorSnapshotState,
        storage: &mut S,
        dist: &Distributions,
        off: usize,
    ) {
        self.builder.start(storage, snapshot);
        if let Some(n) = orig.node_iter(&self.spec.clone()).skip(snapshot.skip_nodes).next() {
            if !self.builder.is_full(storage) {
                self.builder
                    .append_node_mutated_full_bit_flip(&n, off, &self.mutator, storage, dist);
            }
        }
    }

    /// 最低位翻转变异：对节点数据内 off 位置处的字节进行最低位翻转（例如 ^ 0xfe）。
    pub fn mutate_data_lowest_bit_flip<S: GraphStorage>(
        &mut self,
        orig: &VecGraph,
        snapshot: &MutatorSnapshotState,
        storage: &mut S,
        dist: &Distributions,
        off: usize,
    ) {
        self.builder.start(storage, snapshot);
        if let Some(n) = orig.node_iter(&self.spec.clone()).skip(snapshot.skip_nodes).next() {
            if !self.builder.is_full(storage) {
                self.builder
                    .append_node_mutated_lowest_bit_flip(&n, off, &self.mutator, storage, dist);
            }
        }
    }
    

    /// 数值加法扰动变异：对节点数据内 off 位置处的字节进行 wrapping 加法（+ 0x20）。
    pub fn mutate_data_addition<S: GraphStorage>(
        &mut self,
        orig: &VecGraph,
        snapshot: &MutatorSnapshotState,
        storage: &mut S,
        dist: &Distributions,
        off: usize,
    ) {
        self.builder.start(storage, snapshot);
        if let Some(n) = orig.node_iter(&self.spec.clone()).skip(snapshot.skip_nodes).next() {
            if !self.builder.is_full(storage) {
                self.builder
                    .append_node_mutated_addition(&n, off, &self.mutator, storage, dist);
            }
        }
    }

    /// 数值减法扰动变异：对节点数据内 off 位置处的字节进行 (x ^ 0x01).wrapping_sub(0x10) 操作。
    pub fn mutate_data_subtraction<S: GraphStorage>(
        &mut self,
        orig: &VecGraph,
        snapshot: &MutatorSnapshotState,
        storage: &mut S,
        dist: &Distributions,
        off: usize,
    ) {
        self.builder.start(storage, snapshot);
        if let Some(n) = orig.node_iter(&self.spec.clone()).skip(snapshot.skip_nodes).next() {
            if !self.builder.is_full(storage) {
                self.builder
                    .append_node_mutated_subtraction(&n, off, &self.mutator, storage, dist);
            }
        }
    }

    
    
}