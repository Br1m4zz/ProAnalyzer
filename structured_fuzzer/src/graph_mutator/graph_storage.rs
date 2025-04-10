use crate::graph_mutator::graph_iter::{GraphNode, GraphOp, NodeIter, OpIter};
use crate::graph_mutator::newtypes::{DstVal, NodeTypeID, ValueTypeID, OpIndex, PortID, SrcVal};
use crate::graph_mutator::spec::GraphSpec;

use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;

//Needs to be used as a wrapper over Graph Storage, as Graph Storage can't be made into an object due to the Sized constraint
pub trait GraphMutationTarget {
    fn append_op(&mut self, op: u16) -> Option<()>;
    fn append_data(&mut self, data: &[u8]) -> Option<&mut [u8]>;
    fn get_data(&mut self, size: usize) -> Option<&mut [u8]>;
    fn data_available(&self) -> usize;
    fn ops_available(&self) -> usize;
}

impl<T: GraphStorage> GraphMutationTarget for T {
    fn append_op(&mut self, op: u16) -> Option<()> {
        return self.append_op(op);
    }
    fn append_data(&mut self, data: &[u8]) -> Option<&mut [u8]> {
        return self.append_data(data);
    }
    fn get_data(&mut self, size: usize) -> Option<&mut [u8]> {
        return self.get_data(size);
    }
    fn data_available(&self) -> usize {
        return self.data_available();
    }
    fn ops_available(&self) -> usize {
        return self.ops_available();
    }
}

pub trait GraphStorage: Sized {
    fn clear(&mut self);
    fn truncate_to(&mut self, ops_i: usize, data_i: usize);
    fn append_op(&mut self, op: u16) -> Option<()>;
    fn append_data(&mut self, data: &[u8]) -> Option<&mut [u8]>;
    fn get_data(&mut self, size: usize) -> Option<&mut [u8]>;
    fn data_available(&self) -> usize;
    fn ops_available(&self) -> usize;
    fn can_append(&self, node: &GraphNode) -> bool;
    fn data_len(&self) -> usize;
    fn op_len(&self) -> usize;
    fn node_len(&self, spec:&GraphSpec) -> usize;
    fn is_empty(&self) -> bool {
        return self.op_len() == 0;
    }
    fn ops_as_slice(&self) -> &[u16];
    fn data_as_slice(&self) -> &[u8];

    fn as_vec_graph(&self) -> VecGraph {
        let mut ops = Vec::with_capacity(self.op_len());
        let mut data = Vec::with_capacity(self.data_len());
        ops.extend_from_slice(self.ops_as_slice());
        data.extend_from_slice(self.data_as_slice());
        let res = VecGraph::new(ops, data);
        return res;
    }

    fn copy_from(&mut self, graph: &VecGraph)  {
        self.clear();
        for op in graph.ops_as_slice(){
            self.append_op(*op);
        }
        self.append_data(graph.data_as_slice());
    }
    
    /// 取传入的VecGraph的前cutoff个节点
    fn copy_from_cutoff(&mut self, graph: &VecGraph, cutoff:usize, spec: &GraphSpec){
        assert!(cutoff <= graph.op_len(), "ASSERT: cutoff{} should be smaller than input_op_len{}",cutoff, graph.op_len());
        self.clear();
        for op in graph.ops_as_slice().iter().take(cutoff) {
            self.append_op(*op);
        }
        for node in graph.as_vec_graph().node_iter(&spec).take(cutoff){
            self.append_data(node.data);
        }
    }

    fn copy_from_slice_a_b(&mut self, graph: &VecGraph, a:usize,b:usize, spec: &GraphSpec){
        assert!(a < b && b <= graph.op_len(), "ASSERT: Check a b size ");
        self.clear();
        for op in graph.ops_as_slice().iter().skip(a).take(b){
            self.append_op(*op);
        }
        for node in graph.as_vec_graph().node_iter(&spec).skip(a).take(b){
            self.append_data(node.data);
        }
    }
    
    fn calc_edges(&self, spec: &GraphSpec) -> Vec<(SrcVal, DstVal)> {
        let mut res = vec![];
        let mut id_to_src = HashMap::new();
        let mut last_node = None;
        let mut last_out_port = PortID::new(0);
        let mut last_in_port = PortID::new(0);
        let mut last_pass_port = PortID::new(0);
        for (i, op) in self.op_iter(spec).enumerate() {
            match op {
                GraphOp::Node(_n_type_id) => {
                    last_node = Some(OpIndex::new(i));
                    last_in_port = PortID::new(0);
                    last_pass_port = PortID::new(0);
                    last_out_port = PortID::new(0);
                }
                GraphOp::Set(vt, id) => {
                    id_to_src.insert((vt, id), SrcVal::new(last_node.unwrap(), last_out_port));
                    last_out_port = last_out_port.next();
                }
                GraphOp::Get(vt, id) => {
                    res.push((
                        id_to_src.remove(&(vt, id)).unwrap(),
                        DstVal::new(last_node.unwrap(), last_in_port),
                    ));
                    last_in_port = last_in_port.next();
                }
                GraphOp::Pass(vt, id) => {
                    res.push((
                        id_to_src.get(&(vt, id)).cloned().unwrap(),
                        DstVal::new(last_node.unwrap(), last_pass_port),
                    ));
                    last_pass_port = last_pass_port.next();
                }
            }
        }
        return res;
    }

    ///将对应的测试用例转化为文件
    fn write_to_file(&self, path: &str, spec: &GraphSpec) {
        use std::io::BufWriter;
        use std::io::prelude::*;
        let mut file = BufWriter::new(File::create(path).unwrap_or_else(|_| panic!("couldn't open file to dump input {}",path)));
        file.write_all(&spec.checksum.to_le_bytes()).expect("couldn't write checksum");
        file.write_all(&(self.ops_as_slice().len() as u64).to_le_bytes()).expect("couldn't write graph op len");
        file.write_all(&(self.data_as_slice().len() as u64).to_le_bytes()).expect("couldn't write graph data len");
        file.write_all(&(5*8_u64).to_le_bytes()).expect("couldn't write graph op offset");
        file.write_all(&(5*8+(self.ops_as_slice().len() as u64)*2_u64).to_le_bytes()).expect("couldn't write graph data offset");
        for b in self.ops_as_slice().iter(){
            file.write_all(&b.to_le_bytes()).expect("couldn't write graph op");
        }
        file.write_all(self.data_as_slice()).expect("couldn't write graph data");
    }

    fn node_iter<'a>(&'a self, spec: &'a GraphSpec) -> NodeIter<'a> {
        return NodeIter::new(self.ops_as_slice(), self.data_as_slice(), spec);
    }
    fn op_iter<'a>(&'a self, spec: &'a GraphSpec) -> OpIter<'a> {
        return OpIter::new(self.ops_as_slice(), spec);
    }

    ///提供了一个打印ref graph的接口
    fn to_svg(&self, path: &str, spec: &GraphSpec) {
        use std::process::{Command, Stdio};
        let dot = self.to_dot(spec);
        let mut child = Command::new("dot")
            .stdout(Stdio::inherit())
            .stdin(Stdio::piped())
            .arg("-Tsvg")
            .arg("-o")
            .arg(path)
            .spawn()
            .expect("failed to execute dot");
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&dot.as_bytes())
            .expect("failed to write dot graph");
        child.wait().expect("failed to wait on dot");
    }

    fn to_png(&self, path: &str, spec: &GraphSpec) {
        use std::process::{Command, Stdio};
        let dot = self.to_dot(spec);
        let mut child = Command::new("dot")
            .stdout(Stdio::inherit())
            .stdin(Stdio::piped())
            .arg("-Tpng")
            .arg("-o")
            .arg(path)
            .spawn()
            .expect("failed to execute dot");
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(&dot.as_bytes())
            .expect("failed to write dot graph");
        child.wait().expect("failed to wait on dot");
    }

    fn var_names(ops: &[u16], types: &[ValueTypeID], spec: &GraphSpec) -> String{
        let iter = ops.iter().enumerate().map(|(i,id)| format!("v_{}_{}",spec.get_value(types[i]).unwrap().name, id));
        return itertools::Itertools::intersperse(iter, ", ".to_string()).collect::<String>();
    }

    fn write_to_script_file(&self, path: &str, spec: &GraphSpec) {
        let mut file = File::create(path).unwrap();
        file.write_all(self.to_script(spec).as_bytes()).unwrap()
    }

    fn to_script(&self, spec: &GraphSpec) -> String {
        let mut res = "".to_string();
        for n in self.node_iter(spec){
            let node = n.spec.get_node(n.id).unwrap();
            let data = spec.node_data_inspect(n.id, n.data).replace("\\l","");
            let i = 1+node.inputs.len();
            let inputs = Self::var_names(&n.ops[1..i], &node.inputs, spec);
            let j = i + node.passthroughs.len();
            let borrows = Self::var_names(&n.ops[i..j], &node.passthroughs, spec);
            let outputs = Self::var_names(&n.ops[j..], &node.outputs, spec);
            if !outputs.is_empty() {
                res += &outputs;
                res += " = ";
            }
            res+= &format!("{}( inputs=[{}], borrows=[{}], data={})\n",node.name, inputs, borrows, data)
        }
        //println!("{}",res);
        return res;
    }

    fn write_to_dot_file(&self, path: &str, spec: &GraphSpec) {
        let mut file = File::create(path).unwrap();
        file.write_all(self.to_dot(spec).as_bytes()).unwrap()
    }

    fn to_dot(&self, spec: &GraphSpec) -> String {
        let mut res = "digraph{\n rankdir=\"LR\";\n { edge [style=invis weight=100];".to_string();
        let edges = self.calc_edges(spec);
        let mut join = "";
        for (_ntype, i) in self
            .op_iter(&spec)
            .enumerate()
            .filter_map(|(i, op)| op.node().map(|n| (n, i)))
        {
            res += &format!("{}n{}", join, i);
            join = "->";
        }
        res += "}\n";
        for node in self.node_iter(spec) {
            res += &format!(
                "n{} [label=\"{}{}\", shape=box];\n",
                node.op_i,
                spec.get_node(node.id).unwrap().name,
                spec.node_data_inspect(node.id, node.data),
            );
        }
        for (src, dst) in edges.iter() {
            let node_type = NodeTypeID::new(self.ops_as_slice()[src.id.as_usize()]);
            let value_type = spec.get_node(node_type).unwrap().outputs[src.port.as_usize()];
            let edge_type = &spec.get_value(value_type).unwrap().name;
            res += &format!(
                "n{} -> n{} [label=\"{}\"];\n",
                src.id.as_usize(),
                dst.id.as_usize(),
                edge_type
            );
        }
        res += "}";
        //println!("{}", res);
        return res;
    }
}

/// specfuzz中一个测试用例的内存表示方式
/// 
/// （ops操作序列，data数据序列）
/// 
#[derive(Clone)]
pub struct VecGraph {
    ops: Vec<u16>,  //存储操作序列，每个操作序列是一个4字节数据
    data: Vec<u8>,  //存储数据序列，
}

impl VecGraph {

    pub fn new_with_size(op_len: usize, data_len: usize) -> Self{
        return Self::new(vec!(0; op_len), vec!(0; data_len));
    }

    pub fn new(ops: Vec<u16>, data: Vec<u8>) -> Self {
        return Self { ops, data,};
    }

    /// 根据路径path指定的bin文件、spec指定的GraphSpec，生成测试用例对应的VecGraph
    /// 
    /// 输出：path读取的测试用例.bin，使用spec翻译后得到的VecGraph
    /// 
    pub fn new_from_bin_file(path: &str, spec: &GraphSpec) -> VecGraph{

        use std::io::BufReader;
        use std::convert::TryInto;

        //使用BufReader包装一个文件读取器来打开指定路径的.bin文件。
        let mut f: BufReader<File> = BufReader::new(File::open(path).expect("youldn't open .bin file for reading"));
        let mut buffer = [0; 8];
        //读取文件的前8个字节，将其转换为u64类型的校验和，并验证它是否与spec.checksum相匹配。
        let n = f.read(&mut buffer[..]).expect("couldn't read checksum");
        assert_eq!(n,8);
        let checksum = u64::from_le_bytes(buffer.try_into().unwrap());
        assert_eq!(checksum, spec.checksum);

        //读取操作数（num_ops）和数据数（num_data），这些也是以u64类型存储的。
        let n = f.read(&mut buffer[..]).expect("couldn't read num_ops");
        assert_eq!(n,8);
        let num_ops = u64::from_le_bytes(buffer.try_into().unwrap());

        let n = f.read(&mut buffer[..]).expect("couldn't read num_data");
        assert_eq!(n,8);
        let num_data = u64::from_le_bytes(buffer.try_into().unwrap());

        //读取操作（op_offset)和数据的偏移量(data_offset），这些指示了操作和数据在文件中的起始位置。
        let n = f.read(&mut buffer[..]).expect("couldn't read op_offset");
        assert_eq!(n,8);
        let op_offset = u64::from_le_bytes(buffer.try_into().unwrap());

        let n = f.read(&mut buffer[..]).expect("couldn't read data_offset");
        assert_eq!(n,8);
        let data_offset = u64::from_le_bytes(buffer.try_into().unwrap());

        //将文件读取器的位置移动到操作的偏移量处，然后读取每个操作（op），这些操作是以u16类型（4字节）存储的，并将它们添加到VecGraph的操作列表中。
        f.seek(std::io::SeekFrom::Start(op_offset)).unwrap();

        let mut res = VecGraph::empty();    //新建的空VecGraph （res）

        //读取bin中的每个操作到ops
        for _i in 0..num_ops{
            let n = f.read(&mut buffer[..2]).unwrap();
            assert_eq!(n,2);
            let op = u16::from_le_bytes(buffer[..2].try_into().unwrap());
            res.ops.push(op);
        }

        //将文件读取器的位置移动到数据的偏移量处，创建一个新的数据向量res.data，并读取所有数据到这个向量中。
        f.seek(std::io::SeekFrom::Start(data_offset)).unwrap();
        //读取bin中的操作数据到data
        res.data = vec!(0; num_data as usize); //新建保存操作用的数据的向量，大小为num_data
        f.read_exact(&mut res.data[..]).unwrap(); //将数据保存到data中
        return res;
    }

    pub fn empty() -> Self {
        return Self::new(vec![], vec![]);
    }

    pub fn as_ref_graph<'a>(&'a mut self, ops_i: &'a mut usize, data_i: &'a mut usize) -> RefGraph<'a>{
        return RefGraph::new(&mut self.ops[..], &mut self.data[..], ops_i, data_i);
    }

    /// 返回当前 VecGraph 中最后一个节点的数据的长度（单位：字节）
    /// 如果图中没有任何节点，则返回 0
    pub fn get_last_node_data_length(&self, spec: &GraphSpec) -> usize {
        // 使用 node_iter 遍历所有节点，然后取最后一个节点，
        // 如果存在，则解析其 data 的前两个字节获取数据长度，否则返回 0
        self.node_iter(spec)
            .last()
            .and_then(|node| {
                if node.data.len() >= 2 {
                    // 解析前两个字节为数据长度（假设是小端字节序）
                    let length = u16::from_le_bytes([node.data[0], node.data[1]]);
                    Some(length as usize)
                } else {
                    None // 如果数据长度不足两个字节，则返回 None
                }
            })
            .unwrap_or(0) // 如果没有节点或解析失败，则返回 0
    }
    

}

impl GraphStorage for VecGraph {
    fn clear(&mut self) {
        self.ops.clear();
        self.data.clear();
    }
    //根据传入的ops_i和data_i重新设置ops和data大小
    fn truncate_to(&mut self, ops_i: usize, data_i: usize){
        assert!(self.ops.len()>=ops_i);
        assert!(self.data.len()>=data_i);
        self.ops.resize(ops_i, 0);
        self.data.resize(data_i, 0);
    }
    fn append_op(&mut self, op: u16) -> Option<()> {
        self.ops.push(op);
        return Some(());
    }

    fn get_data(&mut self, size: usize) -> Option<&mut [u8]> {
        let len = self.data.len();
        self.data.resize(len + size, 0);
        return Some(&mut self.data[len..]);
    }

    fn append_data(&mut self, data: &[u8]) -> Option<&mut [u8]> {
        self.data.extend_from_slice(data);
        let len = self.data.len();
        return Some(&mut self.data[len - data.len()..]);
    }

    fn data_available(&self) -> usize {
        return 0xffff_ffff;
    }

    fn ops_available(&self) -> usize {
        return 0xffff_ffff;
    }

    fn can_append(&self, _n: &GraphNode) -> bool {
        return true;
    }

    fn op_len(&self) -> usize {
        return self.ops.len();
    }
    
    fn node_len(&self, spec:&GraphSpec) -> usize{
        return self.node_iter(spec).count();
    }

    fn data_len(&self) -> usize {
        return self.data.len();
    }
    fn ops_as_slice(&self) -> &[u16] {
        return &self.ops[..];
    }
    fn data_as_slice(&self) -> &[u8] {
        return &self.data[..];
    }
}

/// refgraph记录了全局的操作图的信息
/// 
/// 包含：操作图ops，操作数据data，一个操作指向指针、一个数据指向指针
/// 
pub struct RefGraph<'a> {
    ops: &'a mut [u16],         //记录图操作码的可变引用，应该记录的是起点
    ops_i: &'a mut usize,       //追踪ops当前位置索引的可变引用
    data: &'a mut [u8],         //记录数据的可变引用，应该记录的是起点
    data_i: &'a mut usize,      //记录data当前数据索引的可变引用
}

///根据传入的数据构造refgraph
impl<'a> RefGraph<'a> {
    pub fn new(
        ops: &'a mut [u16],
        data: &'a mut [u8],
        ops_i: &'a mut usize,
        data_i: &'a mut usize,
    ) -> Self {
        return Self {
            ops,
            ops_i,
            data,
            data_i,
        };
    }

    ///根据payload指示的原始数据和checksum（checksum只是payload的头部信息之一），生成refgraph
    /// 
    /// 
    /// 
    pub fn new_from_slice(payload: &mut [u8], checksum: u64) -> Self {
        let header_len = std::mem::size_of::<u64>() * 5;    //头部长度
        let data_len = payload.len() - header_len;          //数据长度 = payload长度 - 头部长度
        assert_eq!(data_len % 8, 0);
        let buff_size = data_len / 2;                       //buffer的实际大小

        unsafe {
        let ptr = payload.as_mut_ptr();                                                             //获取payload的原始指针ptr
            assert_eq!((ptr as usize) % std::mem::align_of::<u64>(), 0);                                    //确保payload的指针是64位对齐的
            //以下都是payload的头部信息
            let checksum_ptr = (ptr as *mut u64).add(0).as_mut().unwrap();                  //获取checksum_ptr校验和索引
            let ops_i_ptr = (ptr as *mut usize).add(1).as_mut().unwrap();                 //获取ops_i_ptr操作索引
            let data_i_ptr = (ptr as *mut usize).add(2).as_mut().unwrap();                  //获取data_i_ptr数据索引
            assert_eq!(std::mem::size_of::<u64>(), std::mem::size_of::<usize>());                           //确保usize和u64是对齐的
            let graph_offset_ptr = (ptr as *mut u64).add(3).as_mut().unwrap();              //获取graph_offset_ptr图偏移索引
            let data_offset_ptr = (ptr as *mut u64).add(4).as_mut().unwrap();               //获取data_offset_ptr数据偏移索引

            assert_eq!(std::mem::size_of::<usize>(), std::mem::size_of::<u64>());
            *checksum_ptr = checksum;
            *graph_offset_ptr = header_len as u64;
            *data_offset_ptr = (header_len + buff_size) as u64;

            //根据头部信息提供的graph_offset_ptr，计算得到结构体buffer
            let op_ptr = ptr.add(*graph_offset_ptr as usize) as *mut u16;
            let struct_buff = std::slice::from_raw_parts_mut(op_ptr, buff_size / 2);    //获取结构buffer

            //根据头部信息提供的data_offset_ptr，计算得到数据buffer
            let data_ptr = ptr.add(*data_offset_ptr as usize) as *mut u8;
            let data_buff = std::slice::from_raw_parts_mut(data_ptr, buff_size);            //获得数据buffer

            assert_eq!(*data_offset_ptr as usize + buff_size, payload.len());

            return RefGraph::new(struct_buff, data_buff, ops_i_ptr, data_i_ptr);
        }
    }
}

impl<'a> GraphStorage for RefGraph<'a> {
    fn clear(&mut self) {
        *self.ops_i = 0;
        *self.data_i = 0;
    }
    fn truncate_to(&mut self, ops_i: usize, data_i: usize){
        assert!(*self.ops_i>=ops_i);
        assert!(*self.data_i>=data_i);
        *self.ops_i = ops_i;
        *self.data_i = data_i;
    }

    fn append_op(&mut self, op: u16) -> Option<()> {
        if (*self.ops_i as usize) < self.ops.len() {
            self.ops[*self.ops_i as usize] = op;
            *self.ops_i += 1;
            return Some(());
        }
        return None;
    }

    fn get_data(&mut self, size: usize) -> Option<&mut [u8]> {
        if *self.data_i + size <= self.data.len() {
            let range = *self.data_i..*self.data_i + size;
            *self.data_i += size;
            return Some(&mut self.data[range]);
        }
        return None;
    }

    fn append_data(&mut self, data: &[u8]) -> Option<&mut [u8]> {
        if *self.data_i + data.len() <= self.data.len() {
            let range = *self.data_i..(*self.data_i + data.len());
            *self.data_i += data.len();
            let buf = &mut self.data[range];
            buf.copy_from_slice(data);
            return Some(buf);
        }
        return None;
    }

    fn data_available(&self) -> usize {
        return self.data.len() - *self.data_i;
    }
    fn ops_available(&self) -> usize {
        return self.ops.len() - *self.ops_i;
    }

    /// 判断是否可以添加节点
    /// 
    /// 条件：节点n对应的数据长度+ refgraph记录的数据索引data_i大小 < refgraph维护的切片data的数据大小
    /// 
    fn can_append(&self, n: &GraphNode) -> bool {
        return *self.data_i + n.data.len() < self.data.len();
    }

    fn op_len(&self) -> usize {
        return *self.ops_i;
    }

    fn node_len(&self, spec:&GraphSpec) -> usize{
        return self.node_iter(spec).count();
    }

    fn data_len(&self) -> usize {
        return *self.data_i;
    }
    fn ops_as_slice(&self) -> &[u16] {
        return &self.ops[..*self.ops_i];
    }
    fn data_as_slice(&self) -> &[u8] {
        return &self.data[..*self.data_i];
    }
}
