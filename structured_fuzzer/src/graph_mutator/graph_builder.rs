use crate::graph_mutator::graph_iter::{GraphNode, GraphOp, OpIter};
use crate::graph_mutator::graph_storage::{GraphStorage, VecGraph};
use crate::graph_mutator::newtypes::{ConnectorID, GraphError, NodeTypeID, ValueTypeID};
use crate::graph_mutator::spec::{GraphSpec, NodeSpec};
use crate::custom_dict::CustomDict;

use crate::data_buff::DataBuff;
use crate::primitive_mutator::mutator::{PrimitiveMutator,PrimitiveMutatorDefenite};
use crate::random::distributions::Distributions;
use crate::mutator::MutatorSnapshotState;

use std::collections::HashMap;
use std::rc::Rc;

// Implements a set of available values, including the ability to insert, take and update values
// quickly. This is used to construct a graph in the GraphBuilder by inserting nodes individually.
// For every node inserted into the graph, the GraphState is used first to consume the inputs
// needed to create the node, and then to make the outputs available for future nodes. As building
// graphs includes renaming value ids, this also keeps track of the renaming.

// 实现了一组可用values的集合，包括快速插入、获取和更新values的能力。
// 这用于通过单独插入节点来构建GraphBuilder中的图。
// 对于图中插入的每一个节点，GraphState首先被用来处理创建节点所需的输入，
// 并使输出可用于未来的节点。由于构建图包括重命名值的ID，
// 这也跟踪了重命名过程。

#[derive(Debug,Clone)]
struct ValueState {
    cur_id: ConnectorID,
    new_ids_for_sampling: Vec<ConnectorID>, //used to quickly pick an random available id
    new_id_to_ids_index: HashMap<ConnectorID, usize>, //used to remove entries from old_ids_for_sampling vec
    new_id_to_old_id: HashMap<ConnectorID, ConnectorID>,
    old_id_to_new_id: HashMap<ConnectorID, ConnectorID>, //used to look up renamed ids. Will not be cleared if new ids are used, if you have strange memory consumtion issues, look here.
}

impl ValueState {
    pub fn new() -> Self {
        return ValueState {
            cur_id: ConnectorID::new(0),
            new_ids_for_sampling: vec![],
            new_id_to_ids_index: HashMap::new(),
            new_id_to_old_id: HashMap::new(),
            old_id_to_new_id: HashMap::new(),
        };
    }

    ///返回ValueState可用于sampling的id的数量
    fn num_available(&self) -> usize {
        return self.new_ids_for_sampling.len();
    }

    fn clear(&mut self) {
        self.cur_id = ConnectorID::new(0);
        self.new_ids_for_sampling.clear();
        self.new_id_to_ids_index.clear();
        self.old_id_to_new_id.clear();
    }
    
    fn reset_to(&mut self, other: &ValueState){
        self.cur_id = other.cur_id;
        self.new_id_to_ids_index = other.new_id_to_ids_index.clone();
        self.new_ids_for_sampling = other.new_ids_for_sampling.clone();
        self.old_id_to_new_id = other.old_id_to_new_id.clone();
    }

    fn insert_new_id(&mut self) -> ConnectorID {
        self.cur_id = self.cur_id.next();
        let new_id = self.cur_id;
        self.new_ids_for_sampling.push(new_id);
        self.new_id_to_ids_index
            .insert(new_id, self.new_ids_for_sampling.len() - 1);
        return new_id;
    }

    fn insert_old_id(&mut self, old_id: ConnectorID) -> ConnectorID {
        let new_id = self.insert_new_id();
        self.new_id_to_old_id.insert(new_id, old_id);
        self.old_id_to_new_id.insert(old_id, new_id);
        return new_id;
    }

    fn take_new_input(&mut self, new_id: ConnectorID) -> ConnectorID {
        if let Some(i) = self.new_id_to_ids_index.remove(&new_id) {
            //new_id is no longer available for random sampling
            self.new_ids_for_sampling.swap_remove(i);
            if self.new_ids_for_sampling.len() > i {
                self.new_id_to_ids_index
                    .insert(self.new_ids_for_sampling[i], i);
            }
            if let Some(old) = self.new_id_to_old_id.remove(&new_id) {
                self.old_id_to_new_id.remove(&old);
            }
            //this will not remove the old_id -> new_id mapping. If the old id is used without checking, this might cause problems.
            //also this might cause garbage old ids to be collected in the old_id_to_new_id hashmap.
            //Since the builder object is cleared regularly, I believe this will be no big issue.
            return new_id;
        } else {
            panic!("unknown new id: {:?}", new_id);
        }
    }

    fn get_renamed_old_input(&self, old_id: ConnectorID) -> Option<ConnectorID> {
        return self.old_id_to_new_id.get(&old_id).cloned();
    }

    fn take_random_input(&mut self, rng: &Distributions) -> ConnectorID {
        let id = self.get_random_input(rng);
        return self.take_new_input(id);
    }

    fn get_random_input(&mut self, rng: &Distributions) -> ConnectorID {
        if self.new_ids_for_sampling.is_empty() {
            panic!("couldn get random input");
        }
        let index = rng.gen_range(0, self.new_ids_for_sampling.len());
        return self.new_ids_for_sampling[index];
    }

    fn take_old_available_input(
        &mut self,
        old_id: ConnectorID,
        rng: &Distributions,
    ) -> ConnectorID {
        let id = self.get_old_available_input(old_id, rng);
        return self.take_new_input(id);
    }

    fn get_old_available_input(&mut self, old_id: ConnectorID, rng: &Distributions) -> ConnectorID {
        return self
            .get_renamed_old_input(old_id)
            .unwrap_or_else(|| self.get_random_input(rng));
    }
}

///记录操作支持的操作数据的类型的ValueTypeID到ValueState的映射
#[derive(Clone)]
pub struct GraphState {
    v_type_to_state: HashMap<ValueTypeID, ValueState>,
}
impl GraphState {
    ///新建一张哈希表，记录从value type到value state的哈希表映射
    fn new() -> Self {
        return Self {
            v_type_to_state: HashMap::new(),
        };
    }

    fn get_state_mut(&mut self, spec: &ValueTypeID) -> &mut ValueState {
        if !self.v_type_to_state.contains_key(spec) {
            self.v_type_to_state.insert(*spec, ValueState::new());
        }
        return self.v_type_to_state.get_mut(spec).unwrap();
    }

    fn clear(&mut self) {
        for (_, s) in self.v_type_to_state.iter_mut() {
            s.clear();
        }
    }
    //将状态重置为other对应的状态
    fn reset_to(&mut self, other: &GraphState){
        for (v_type, s) in self.v_type_to_state.iter_mut() {
            s.reset_to(&other.v_type_to_state[v_type]);
        }
    }

    pub fn info(&self, spec: &GraphSpec) -> String {
        let mut res = String::new();
        for (vt, state) in self.v_type_to_state.iter() {
            res += &format!(
                "{}: {}\n",
                spec.get_value(*vt).unwrap().name,
                state.num_available()
            );
        }
        return res;
    }

    ///新增节点n是否要添加到操作图的核心判断
    /// 
    /// 条件是：spec中有这样的操作节点，其对应spec节点涉及的每个required_values类型对应所需的cnt在values类型可分配的范围内
    /// 
    fn is_available(&self, spec: &GraphSpec, n: NodeTypeID) -> bool {
        //获取n对应的实际spec节点node_spec
        if let Ok(node_spec) = spec.get_node(n) {
            //遍历这个node_spec的每一个required_values，获得其数据类型v_type和所需的资源数量cnt
            for (v_type, cnt) in node_spec.required_values.iter() {
                //
                //这是一个链式调用，
                //它尝试从self.v_type_to_state映射中获取v_type对应的ValueState。
                //如果ValueState存在，则使用map方法来检查ValueState中可用的数量是否大于或等于所需的数量cnt。
                //如果映射中没有v_type的条目，则unwrap_or(false)确保is_available为false。
                let is_available = self
                    .v_type_to_state
                    .get(v_type)
                    .map(|state| state.num_available() >= *cnt)
                    .unwrap_or(false);
                if !is_available {
                    return false;
                }
            }
            return true;
        }
        return false;
    }
}

pub struct GraphBuilder {
    pub spec: Rc<GraphSpec>,    //使用的spec（智能指针）
    state: GraphState,          //由spec构建的已知操作序列图。使用哈希表组织，描述了从ValueTypeID到ValueState的映射
}

/// 用于存储 spec graph, 
/// 
/// 他的构建是逐个节点构建的
impl GraphBuilder {
    pub fn new(spec: Rc<GraphSpec>) -> Self {
        return Self {
            spec,
            state: GraphState::new(),
        };
    }

    /// 从起点开始，根据snapshot提供的信息重置GraphBuilder记录的哈希表
    /// 
    /// 若snapshot中没提供prefix_graph_state信息（还没拍摄），则清空当前GraphBuilder记录的哈希表
    /// 
    /// 若snapshot有提供prefix_graph_state信息（快照拍摄完成），则恢复GraphBuilder哈希表为prefix_graph_state记录的哈希表
    /// 
    pub fn start<S: GraphStorage>(&mut self, graph: &mut S, snapshot: &MutatorSnapshotState) {
        //根据拍摄快照记录的skip_ops和skip_data从起点截断graph
        graph.truncate_to(snapshot.skip_ops, snapshot.skip_data);
        if let Some(ref state) = snapshot.prefix_graph_state{
            self.state.reset_to(&state);    //如果快照记录中，有prefix_graph_state，那么重置为snapshot记录的prefix_graph_state的state
        } else {//否则清空图记录的状态
            self.state.clear();
        }
    }

    pub fn num_ops_used<S: GraphStorage>(&self, graph: &S) -> usize {
        return graph.ops_as_slice().len();
    }

    fn pick_available_type(&mut self, dist: &Distributions) -> Result<NodeTypeID, GraphError> {
        let state = &self.state;
        let spec = &self.spec;
        let iter = self
            .spec
            .node_specs
            .iter()
            .filter(|n| state.is_available(spec, n.id) && n.generatable)
            .map(|n| n.id);
        return dist
            .choose_from_iter(iter)
            .ok_or(GraphError::InvalidSpecs);
    }

    /// 重命名并链接一个op
    /// 
    /// 
    /// 
    pub fn rename_and_relink_op_val(
        state: &mut GraphState,
        rng: &Distributions,
        op: GraphOp,
    ) -> u16 {
        let newval = match op {
            GraphOp::Get(vtype, input_id) => state
                .get_state_mut(&vtype)
                .take_old_available_input(input_id, rng)
                .as_u16(),
            GraphOp::Set(vtype, con) => state.get_state_mut(&vtype).insert_old_id(con).as_u16(),
            GraphOp::Pass(vtype, input_id) => state
                .get_state_mut(&vtype)
                .get_old_available_input(input_id, rng)
                .as_u16(),
            GraphOp::Node(nt) => nt.as_u16(),
        };
        return newval;
    }

    ///将frag中的ops（也称作slice） 添加到refgraph中
    /// 
    /// 
    /// 
    pub fn append_slice<S: GraphStorage>(&mut self, frag: &[u16], graph: &mut S, dist: &Distributions) {
        for op in OpIter::new(frag, &self.spec) {
            let new_val = Self::rename_and_relink_op_val(&mut self.state, &dist, op);
            graph.append_op(new_val);
        }
    }

    pub fn get_graph_state(&self) -> GraphState{
        return self.state.clone();
    }

    ///判断当前的GraphStorage的节点node之后是否可以添加
    /// 
    /// 条件是：当前资源足够为该节点需要的数据类型分配资源（is_available），且图也可以append(can_append)【基本与当前内存是否能分配有关】
    /// 
    pub fn can_append_node<S: GraphStorage>(&self, node: &GraphNode, graph: &S) -> bool {
        if self.state.is_available(&self.spec, node.id) {
            if graph.can_append(node) {
                return true;
            }
        }
        return false;
    }

    /// 在GraphStorage中添加节点node
    /// 
    /// 输入：准备添加新节点的前面的node、被处理的GraphStorage、要添加的节点使用的变异分布
    /// 
    pub fn append_node<'a, S: GraphStorage>(
        &mut self,
        node: &GraphNode,
        graph: &'a mut S,
        dist: &Distributions
    ) -> Option<DataBuff<'a>> {
        if self.can_append_node(node, graph) {
            self.append_slice(&node.ops, graph, dist);  //添加node中对应的ops
            let data = graph.append_data(node.data).unwrap();   //更新node对应的data
            return Some(DataBuff::new(data, data.len()));   //返回对应的data指针和buffer大小
        }
        return None;
    }

    ///在满足特定条件下，向图形存储结构中添加一个经过变异的节点
    /// 
    pub fn append_node_mutated<S: GraphStorage>(
        &mut self,
        node: &GraphNode,
        dict: &CustomDict,
        mutator: &PrimitiveMutator,
        graph: &mut S,
        dist: &Distributions
    ) {
        if self.can_append_node(node, graph) {
            self.append_slice(&node.ops, graph, dist); //添加node对应的ops
            let ntype = self.spec.get_node(node.id).unwrap(); //获取node的node_specs的节点
            if let Some(dtype) = ntype.data {
                if let Ok(dat) = self.spec.get_data(dtype) {//为这个新节点增加变异后的数据
                    dat.atomic_type
                        .append_mutated(node.data, dict, graph, &self.spec, mutator, dist);
                } else {
                    panic!("Node {} has invalid data type {:?}", ntype.name, ntype.data);
                }
            }
        }
    }

    pub fn finalize<S: GraphStorage>(&self, graph: &S) -> VecGraph {
        return graph.as_vec_graph();
    }

    pub fn drop_node_at<S: GraphStorage>(&mut self, frag: &VecGraph, index: usize, graph: &mut S, dist: &Distributions) {
        self.start(graph, &MutatorSnapshotState::none());
        for (i, node) in frag.node_iter(&self.spec).enumerate() {
            if i != index {
                // basicall inlined append_node to satisfy the borrow checker...
                if self.can_append_node(&node, graph) {
                    for op in OpIter::new(&node.ops, &self.spec) {
                        let new_val =
                            Self::rename_and_relink_op_val(&mut self.state, &dist, op);
                        graph.append_op(new_val);
                    }
                    graph.append_data(node.data);
                }
            }
        }
    }

    pub fn is_full<S: GraphStorage>(&self, graph: &S) -> bool{
        let data_full = self.spec.biggest_data() >= graph.data_available();
        let ops_full = self.spec.biggest_ops() >= graph.ops_available();
        return data_full || ops_full;
    }

    fn can_generate_node<S: GraphStorage>(&self, ntype: &NodeSpec, graph: &S) -> bool {
        if ntype.size() + 1 > graph.ops_available() {
            return false;
        }
        if ntype.min_data_size(&self.spec) > graph.data_available() {
            return false;
        }
        return true;
    }

    ///添加随机的节点操作
    pub fn append_random_node<S: GraphStorage>(
        &mut self,
        node: NodeTypeID,
        mutator: &PrimitiveMutator,
        graph: &mut S,
        dist: &Distributions
    ) -> Result<(), GraphError> {
        let ntype = self.spec.get_node(node)?;
        if !self.can_generate_node(ntype, graph) {
            return Ok(());
        }
        graph.append_op(node.as_u16());
        for vtype in ntype.inputs.iter() {
            let id = self
                .state
                .get_state_mut(&vtype)
                .take_random_input(&dist);
            graph.append_op(id.as_u16());
        }
        for vtype in ntype.passthroughs.iter() {
            let id = self
                .state
                .get_state_mut(&vtype)
                .get_random_input(&dist);
            graph.append_op(id.as_u16());
        }
        for vtype in ntype.outputs.iter() {
            let id = self.state.get_state_mut(&vtype).insert_new_id();
            graph.append_op(id.as_u16());
        }
        if let Some(dtype) = ntype.data {
            if let Ok(dat) = self.spec.get_data(dtype) {
                dat.atomic_type.append_generate(graph, &self.spec, mutator, dist);
            } else {
                panic!(
                    "Node {} {:?} has invalid data type {:?}",
                    ntype.name, node, ntype.data
                );
            }
        }
        return Ok(());
    }

    pub fn minimize<F, S1: GraphStorage, S2: GraphStorage>(
        &mut self,
        frag: &S1,
        storage: &mut S2,
        dist: &Distributions,
        mut tester: F,
    ) -> VecGraph
    where
        F: FnMut(&S2, &GraphSpec) -> bool,
    {
        let mut min_graph = frag.as_vec_graph();
        let num_nodes = frag.node_len(&self.spec);
        for idx in (0..num_nodes).rev() {
            self.drop_node_at(&min_graph, idx, storage, dist);
            if tester(&storage, &self.spec) {
                min_graph = storage.as_vec_graph();
            }
        }
        return min_graph;
    }

    ///添加n个随机的节点
    pub fn append_random<S: GraphStorage>(
        &mut self,
        n: usize,
        mutator: &PrimitiveMutator,
        graph: &mut S,
        dist: &Distributions
    ) -> Result<(), GraphError> {
        for _i in 0..n {
            if self.is_full(graph){break;}
            let n_type = self.pick_available_type(dist)?;
            self.append_random_node(n_type, mutator, graph, dist)?;
        }
        return Ok(());
    }
    
    /// 为节点添加全比特翻转变异数据
    pub fn append_node_mutated_full_bit_flip<S: GraphStorage>(
        &mut self,
        node: &GraphNode,
        off: usize,
        mutator: &PrimitiveMutatorDefenite,
        graph: &mut S,
        dist: &Distributions
    ) {
        if self.can_append_node(node, graph) {
            // 将节点对应的操作添加到图中
            self.append_slice(&node.ops, graph, dist);
            // 获取该节点的类型说明
            let ntype = self.spec.get_node(node.id).unwrap();
            // 如果节点定义中有 data 字段，则获取对应的 Data 对象
            if let Some(dtype) = ntype.data {
                if let Ok(dat) = self.spec.get_data(dtype) {
                    // 调用相应的接口方法，实现全比特翻转变异，
                    // off 参数表示在数据中的偏移位置
                    dat.atomic_type.append_mutated_full_bit_flip(
                        node.data, graph, &self.spec, mutator, off
                    );
                } else {
                    panic!("Node {} has invalid data type {:?}", ntype.name, ntype.data);
                }
            }
        }
    }

    /// 为节点添加最低位翻转变异数据
    pub fn append_node_mutated_lowest_bit_flip<S: GraphStorage>(
        &mut self,
        node: &GraphNode,
        off: usize,
        mutator: &PrimitiveMutatorDefenite,
        graph: &mut S,
        dist: &Distributions
    ) {
        if self.can_append_node(node, graph) {
            self.append_slice(&node.ops, graph, dist);
            let ntype = self.spec.get_node(node.id).unwrap();
            if let Some(dtype) = ntype.data {
                if let Ok(dat) = self.spec.get_data(dtype) {
                    dat.atomic_type.append_mutated_lowest_bit_flip(
                        node.data, graph, &self.spec, mutator, off
                    );
                } else {
                    panic!("Node {} has invalid data type {:?}", ntype.name, ntype.data);
                }
            }
        }
    }





    /// 为节点添加数值加法扰动变异数据
    pub fn append_node_mutated_addition<S: GraphStorage>(
        &mut self,
        node: &GraphNode,
        off: usize,
        mutator: &PrimitiveMutatorDefenite,
        graph: &mut S,
        dist: &Distributions
    ) {
        if self.can_append_node(node, graph) {
            self.append_slice(&node.ops, graph, dist);
            let ntype = self.spec.get_node(node.id).unwrap();
            if let Some(dtype) = ntype.data {
                if let Ok(dat) = self.spec.get_data(dtype) {
                    dat.atomic_type.append_mutated_addition(
                        node.data, graph, &self.spec, mutator, off
                    );
                } else {
                    panic!("Node {} has invalid data type {:?}", ntype.name, ntype.data);
                }
            }
        }
    }

    /// 为节点添加数值减法扰动变异数据
    pub fn append_node_mutated_subtraction<S: GraphStorage>(
        &mut self,
        node: &GraphNode,
        off: usize,
        mutator: &PrimitiveMutatorDefenite,
        graph: &mut S,
        dist: &Distributions
    ) {
        if self.can_append_node(node, graph) {
            self.append_slice(&node.ops, graph, dist);
            let ntype = self.spec.get_node(node.id).unwrap();
            if let Some(dtype) = ntype.data {
                if let Ok(dat) = self.spec.get_data(dtype) {
                    dat.atomic_type.append_mutated_subtraction(
                        node.data, graph, &self.spec, mutator, off
                    );
                } else {
                    panic!("Node {} has invalid data type {:?}", ntype.name, ntype.data);
                }
            }
        }
    }





}
