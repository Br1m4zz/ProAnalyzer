use crate::graph_mutator::atomic_data::AtomicDataType;
use crate::graph_mutator::newtypes::{AtomicTypeID, GraphError, NodeTypeID, ValueTypeID};
use std::collections::HashMap;
use std::sync::Arc;


///
/// specfuzz中用于记录spec的不同节点信息的数据结构，唯一对应一个校验和checksum
/// 
/// 维护三种spec的队列：node_specs、value_specs、data_specs
/// 
/// 快照节点有一个单独的：snapshot_node_id
/// 
/// 数据结构也记录了max_data与max_ops
/// 
#[derive(Default,Clone)]
pub struct GraphSpec {
    pub checksum: u64,                              //记录了当前版本的spec操作图
    pub node_specs: Vec<NodeSpec>,                  //管理所有的spec节点。node_type每新建一个节点就会加一个
    pub value_specs: Vec<ValueSpec>,                //管理所有的ValueSpec节点。value_type每新建一个节点就会加一个
    pub data_specs: Vec<AtomicSpec>,                //管理所有的AtomicSpec节点。data_type每新建一个节点就会加一个
    pub snapshot_node_id: Option<NodeTypeID>,       //记录图中name是“create_tmp_snapshot”的NodeTypeID
    max_data: usize,
    max_ops: usize,                                 //specgraph中涉及的操作数量
}

impl GraphSpec {

    ///新建一个空的specGraph
    pub fn new() -> Self {
        return Self {
            checksum: 0,
            node_specs: Vec::new(),
            value_specs: Vec::new(),
            data_specs: Vec::new(),
            snapshot_node_id: None,
            max_data: 0,
            max_ops: 0,
        };
    }

    ///获取GraphSpec的max_data
    pub fn biggest_data(&self) -> usize { return self.max_data; }

    ///获取GraphSpec的max_ops
    pub fn biggest_ops(&self) -> usize { return self.max_ops; }

    ///根据传入的NodeTypeID获取node_specs的节点
    pub fn get_node(&self, n: NodeTypeID) -> Result<&NodeSpec, GraphError> {
        return self
            .node_specs
            .get(n.as_usize())
            .ok_or(GraphError::UnknownNodeType(n));
    }

    ///根据传入的NodeTypeID，获取对应的value_spec
    pub fn get_value(&self, v: ValueTypeID) -> Result<&ValueSpec, GraphError> {
        return self
            .value_specs
            .get(v.as_usize())
            .ok_or(GraphError::UnknownValueType(v));
    }
    
    ///根据传入的AtomicTypeID，获取data_spec对应的AtomicSpec
    pub fn get_data(&self, v: AtomicTypeID) -> Result<&AtomicSpec, GraphError> {
        return self
            .data_specs
            .get(v.as_usize())
            .ok_or(GraphError::UnknownDataType(v));
    }

    pub fn get_node_size(&self, n: NodeTypeID) -> Result<usize, GraphError> {
        return Ok(self.get_node(n)?.size());
    }

    /// 在specGraph中，根据name，注册一个新的ValueSpec，并确保每个ValueType都有一个唯一的标识符。新增的ValueSpec被记录在value_specs中
    /// 
    /// 输出：注册的新的ValueType的id（ValueTypeID—）
    /// 
    pub fn value_type(&mut self, name: &str) -> ValueTypeID {
        assert!(self.value_specs.len() < std::u16::MAX as usize);
        let new_id = ValueTypeID::new(self.value_specs.len() as u16);
        self.value_specs.push(ValueSpec::new(name, new_id));
        return new_id;
    }

    /// 在specGraph中，根据name，注册一个新的AtomicSpec，确保每种数据类型都有一个唯一的标识符
    /// 
    /// 输出：注册的新的AtomicTypeID的id（AtomicTypeID—）
    pub fn data_type(&mut self, name: &str, atom: Arc<dyn AtomicDataType+Send+Sync>) -> AtomicTypeID {
        let new_id = AtomicTypeID::new(self.data_specs.len());
        if self.max_data < atom.min_data_size() {self.max_data = atom.min_data_size(); }
        let spec = AtomicSpec::new(name, new_id, atom);
        self.data_specs.push(spec);
        return new_id;
    }

    /// 用于构建和注册图形节点类型
    /// 
    /// 输入：节点命名name，节点数据data，输入inputs，passthrough，输出outputs
    /// 
    /// 输出：该节点类型对应的NodeTypeID
    /// 
    pub fn node_type(
        &mut self,
        name: &str,
        data: Option<AtomicTypeID>,
        inputs: Vec<ValueTypeID>,
        passthrough: Vec<ValueTypeID>,
        outputs: Vec<ValueTypeID>,
    ) -> NodeTypeID {
        assert!(self.node_specs.len() < std::u16::MAX as usize);//验证当前的节点规范数量在限定范围内
        let new_id = NodeTypeID::new(self.node_specs.len() as u16); //创建一个新的节点类型ID
        let ops_len = inputs.len() + passthrough.len() + outputs.len()+1; //计算操作长度（ops_len），它是输入passthrough、输出的长度之和加一。

        //如果当前最大操作数（self.max_ops）小于前面计算的操作长度ops_len，则更新最大操作数为ops_len。
        if self.max_ops < ops_len {self.max_ops = ops_len;}

        //根据传入的参数，创建一个新的节点规范（NodeSpec）
        let spec = NodeSpec::new(name, new_id, data, inputs, passthrough, outputs);

        //把对应的节点加入到node_specs中
        self.node_specs.push(spec);

        //如果新建节点名称是"create_tmp_snapshot"，首先断言确认操作长度=1，并更新snapshot_node_id为新创建的节点的new_id
        if name == "create_tmp_snapshot" {
            assert_eq!(ops_len,1);
            self.snapshot_node_id = Some(new_id);
        }
        return new_id;
    }

    ///对给定节点类型ID（NodeTypeID）和关联的数据（data）查看特定节点的数据内容，并返回对应数据atom的数据串。
    /// 
    /// 输入：spec节点ID（NodeTypeID），
    /// 
    pub fn node_data_inspect(&self,n: NodeTypeID, data:&[u8]) -> String{
        //使用get_node方法尝试根据节点类型NodeTypeID寻找对应的节点规范NodeSpec。
        let node = self.get_node(n).unwrap();
        //如果节点包含数据（node.data），则使用get_data方法获取与atom_id相对应的原子（atom）
        if let Some(atom_id) = node.data {
            let atom = self.get_data(atom_id).unwrap();
            return format!("{}",atom.atomic_type.data_inspect(data, self) );
        }
        //如果节点不包含数据，函数返回一个空字符串
        return "".to_string();
    }
}

///用于管理测试用例数据的的数据结构
/// 
/// 
#[derive(Clone)]
pub struct AtomicSpec {
    pub name: String,
    pub id: AtomicTypeID,
    pub atomic_type: Arc<dyn AtomicDataType+Send+Sync>,
}

impl AtomicSpec {
    ///新建一个空的数据spec
    pub fn new(name: &str, id: AtomicTypeID, atomic_type: Arc<dyn AtomicDataType+Send+Sync>) -> Self {
        return AtomicSpec {
            name: name.to_string(),
            id,
            atomic_type,
        };
    }
}


/// 记录不同类型的节点的数据结构
/// 
/// 
/// 
#[derive(Clone)]
pub struct NodeSpec {
    pub name: String,
    pub id: NodeTypeID,
    pub inputs: Vec<ValueTypeID>,
    pub outputs: Vec<ValueTypeID>,
    pub passthroughs: Vec<ValueTypeID>,
    pub required_values: HashMap<ValueTypeID, usize>,
    pub data: Option<AtomicTypeID>,
    pub generatable:bool,
}

impl NodeSpec {
    ///根据传入的名称，设定id，数据，输入，passthrough和输出构建这个NodeSpec
    fn new(
        name: &str,
        id: NodeTypeID,
        data: Option<AtomicTypeID>,
        inputs: Vec<ValueTypeID>,
        passthroughs: Vec<ValueTypeID>,
        outputs: Vec<ValueTypeID>,
    ) -> Self {
        let mut required_values = HashMap::new();

        for pass in passthroughs.iter() {
            if *required_values.entry(*pass).or_insert(0) == 0 {
                *required_values.entry(*pass).or_insert(0) = 1;
            }
        }
        for inp in inputs.iter() {
            *required_values.entry(*inp).or_insert(0) += 1;
        }

        return Self {
            name: name.to_string(),
            id,
            inputs,
            outputs,
            passthroughs,
            required_values,
            data,
            generatable: name != "create_tmp_snapshot"
        };
    }

    pub fn size(&self) -> usize {
        return self.inputs.len() + self.passthroughs.len() + self.outputs.len();
    }

    pub fn min_data_size(&self, spec: &GraphSpec) -> usize {
        return self
            .data
            .map(|d| spec.get_data(d).unwrap())
            .map(|spec| spec.atomic_type.min_data_size())
            .unwrap_or(0);
    }
}

#[derive(Clone)]
pub struct ValueSpec {
    pub id: ValueTypeID,
    pub name: String,
}

impl ValueSpec {
    pub fn new(name: &str, id: ValueTypeID) -> Self {
        return Self {
            name: name.to_string(),
            id,
        };
    }
}
