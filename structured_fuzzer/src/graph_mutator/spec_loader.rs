//use rmps::{Deserializer, Serializer};
use serde::{Deserialize, Serialize};

use std::io::Read;
use std::sync::Arc;

use crate::graph_mutator::atomic_data::{DataInt, DataStruct, DataVec};
use crate::graph_mutator::newtypes::{AtomicTypeID, ValueTypeID};
use crate::graph_mutator::spec::GraphSpec;
use crate::graph_mutator::generators::{IntGenerator, VecGeneratorLoader};

#[derive(Debug, PartialEq, Deserialize, Serialize)]
struct EdgeLoader {
    name: String,
}

#[derive(Debug, PartialEq, Deserialize, Serialize)]
struct NodeLoader {
    name: String,
    atom_id: Option<usize>,
    inputs: Vec<u16>,
    borrows: Vec<u16>,
    outputs: Vec<u16>,
    is_interactive: bool,
}


/// 设计有三种不同的Atom：Struct、Int、Vec
/// 
/// 
/// 
#[derive(Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type")]
enum AtomLoader {
    Struct {
        name: String,
        fields: Vec<(String, usize)>,
    },
    Int {
        name: String,
        size: usize,
        generators: Vec<IntGenerator>,
    },
    Vec {
        name: String,
        size_range: (usize, usize),
        dtype: usize,
        generators: Vec<VecGeneratorLoader>,
    },
}

/// 用于记录并转化输入数据为节点、边、原子的数据结构，
/// 
/// 本数据结构用于转化为specgraph
#[derive(Debug, PartialEq, Deserialize, Serialize)]
struct SpecLoader {
    checksum: u64,
    nodes: Vec<NodeLoader>,
    edges: Vec<EdgeLoader>,
    atomics: Vec<AtomLoader>,
}

/// 主要实现to_graph_spec的方法
impl SpecLoader {

    ///根据已有的数据创建GraphSpec，并返回之
    fn to_graph_spec(mut self) -> GraphSpec {
        let mut g = GraphSpec::new();
        g.checksum = self.checksum;
        self.atoms_to_graphspec(&mut g);
        self.edges_to_graphspec(&mut g);
        self.nodes_to_graphspec(&mut g);
        return g;
    }

    ///遍历 SpecLoader 中的原子数据类型集合 atomics，根据每个原子数据类型的具体种类（结构体、整数、向量），
    /// 
    /// 创建相应的 DataStruct、DataInt 或 DataVec 实例，
    /// 
    /// 并使用 GraphSpec 的 data_type 方法将它们添加到 GraphSpec 中
    /// 
    /// atomics对应着GraphSpec的data_type
    /// 
    fn atoms_to_graphspec(&mut self, g: &mut GraphSpec) {
        for (i, atom) in self.atomics.iter().enumerate() {
            match atom {
                AtomLoader::Struct { name, fields } => {
                    let fields = fields
                        .into_iter()
                        .map(|(n, id)| (n.clone(), AtomicTypeID::new(*id)))
                        .collect::<Vec<_>>();
                    let struct_def = DataStruct::new(fields, &g);
                    let id = g.data_type(&name, Arc::new(struct_def));
                    assert_eq!(id.as_usize(), i);
                }
                AtomLoader::Int { name, size, generators } => {
                    let id = g.data_type(&name, Arc::new(DataInt::new(*size, generators.to_vec())));
                    assert_eq!(id.as_usize(), i);
                }
                AtomLoader::Vec {
                    name,
                    size_range: rng,
                    dtype,
                    generators,
                } => {
                    let dtype = AtomicTypeID::new(*dtype);
                    let dspec = g
                        .get_data(dtype)
                        .expect(&format!("invalid data type id ({:?}) used in vec", dtype));
                    assert!(dspec.atomic_type.size().is_fixed());
                    let generators = generators.into_iter().map(|g| g.load(dspec)).collect();
                    let id = g.data_type(&name, Arc::new(DataVec::new(*rng, dtype, generators, &g)));
                    assert_eq!(id.as_usize(), i);
                }
            }
        }
    }

    ///遍历 SpecLoader 中的边集合 edges，并为每条边调用 GraphSpec 的 value_type 方法，将边的名称添加到 GraphSpec 中
    /// 
    /// 边对应着GraphSpec的value_type
    /// 
    fn edges_to_graphspec(&mut self, g: &mut GraphSpec) {
        for (i, edge) in self.edges.iter().enumerate() {
            let id = g.value_type(&edge.name);
            assert_eq!(id.as_usize(), i);
        }
    }

    ///遍历 SpecLoader 中的节点集合nodes，为每个节点创建一个新的节点类型，并将节点的数据、输入、借用和输出添加到 GraphSpec 中。
    /// 
    /// 节点对应着node_type
    /// 
    fn nodes_to_graphspec(&mut self, g: &mut GraphSpec) {
        for (i, node) in self.nodes.iter().enumerate() {
            let data = node.atom_id.map(|i| AtomicTypeID::new(i));
            let inputs: Vec<ValueTypeID> =
                node.inputs.iter().map(|i| ValueTypeID::new(*i)).collect();
            let borrows: Vec<ValueTypeID> =
                node.borrows.iter().map(|i| ValueTypeID::new(*i)).collect();
            let outputs: Vec<ValueTypeID> =
                node.outputs.iter().map(|i| ValueTypeID::new(*i)).collect();
            let id = g.node_type(&node.name, data, inputs, borrows, outputs);
            assert_eq!(id.as_usize(), i);
        }
    }
}

/// 对传入的数据data进行反序列化，得到对应的SpecLoader，并构建SpecLoader对应的测试用例的specgraph
pub fn load_spec_from_read<R: Read>(data: R) -> GraphSpec {
    let l: SpecLoader = rmp_serde::from_read(data).unwrap();
    return l.to_graph_spec();
}

///测试反序列化构建spec graph的结果
#[cfg(test)]
mod tests {
    use super::*;
    use crate::graph_mutator::graph_storage::VecGraph;
    use crate::primitive_mutator::mutator::PrimitiveMutator;
    use crate::GraphBuilder;
    use crate::mutator::MutatorSnapshotState;
    use std::fs::File;
    use std::rc::Rc;

    // #[test]
    // fn test_export() {
    //     let val = SpecLoader {
    //         atomics: vec![AtomLoader::Int {
    //             name: "foo".into(),
    //             size: 3,
    //             generators: vec!()
    //         }],
    //         checksum: 1337,
    //         edges: vec![],
    //         nodes: vec![],
    //     };

    //     let mut buf = vec![];
    //     //let mut buf = File::create("nsgpack_test.msgp").unwrap();
    //     val.serialize(&mut serde::Serializer::new(&mut buf)).unwrap();

    //     let file = File::open("interpreter/build/spec.msgp").unwrap();
    //     let l2: SpecLoader = rmp_serde::from_read(file).unwrap();

    //     println!("{:?}", l2);
    // }

    // #[test]
    // fn test_import() {
    //     let file = File::open("interpreter/build/spec.msgp").unwrap();
    //     let g = load_spec_from_read(file);
    //     let d = crate::random::distributions::Distributions::new(vec!());
    //     let mut gb = GraphBuilder::new(Rc::new(g),);
    //     let mutator = PrimitiveMutator::new();
    //     let mut st = VecGraph::new(vec![], vec![]);
    //     gb.start(&mut st, &MutatorSnapshotState::none());
    //     gb.append_random(10, &mutator, &mut st, &d).unwrap();
    // }
}
