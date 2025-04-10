use std::sync::Arc;
//use std::sync::RwLock;
use std::time::Duration;

use crate::structured_fuzzer::custom_dict::CustomDict;
use crate::bitmap::{Bitmap, StorageReason};
use crate::fuzz_runner::ExitReason;
use crate::structured_fuzzer::graph_mutator::graph_storage::VecGraph;
use crate::structured_fuzzer::mutator::MutationStrategy;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct InputID(usize);// 定义一个包含单个usize字段的结构体

impl InputID {
    /// 创建一个新的InputID实例
    pub fn new(a: usize) -> Self {
        Self(a)
    }
    /// 创建一个表示无效ID的InputID实例
    pub fn invalid() -> Self {
        Self(std::usize::MAX)
    }
    /// 获取InputID内部的usize值
    pub fn as_usize(&self) -> usize {
        self.0
    }
}


///输入的处理状态就两种：
/// 
/// - Minimize：要做最小化处理
/// 
/// - Havoc：要做HAVOC变异处理
#[derive(Clone)]
pub enum InputState {
    Minimize,
    Havoc,
}

/// 一个测试用例实例
/// 
/// 包含了其内容，对应的响应
/// 
#[derive(Clone)]
pub struct Input {
    pub id: InputID,
    pub data: Arc<VecGraph>,    // 输入数据的共享智能指针
    pub bitmap: Bitmap,         // 该输入的bitmap
    pub exit_reason: ExitReason,    // 该输入的退出原因
    pub ops_used: usize,            // 使用的操作数
    pub time: Duration,             // 处理时间
    pub storage_reasons: Vec<StorageReason>,    // 存储原因
    pub found_by: MutationStrategy,     // 发现该输入的变异策略
    pub state: InputState,  // 输入的状态
    pub custom_dict: CustomDict,
    pub parent_snapshot_position: usize, //记录其父种子使用的快照位置
    pub parent_id:InputID//记录其父种子
}

impl Input {
    pub fn new(
        data: VecGraph,                      
        found_by: MutationStrategy,           
        storage_reasons: Vec<StorageReason>,    
        bitmap: Bitmap,                        
        exit_reason: ExitReason,
        ops_used: usize,
        time: Duration,
    ) -> Self {
        return Self {
            id: InputID::invalid(),
            data: Arc::new(data),
            bitmap,
            storage_reasons,
            exit_reason,
            time,
            state: InputState::Minimize,
            found_by,
            ops_used,
            custom_dict: CustomDict::new(),
            parent_snapshot_position: 0,
            parent_id: InputID::invalid(),
        };
    }

    // pub fn update_parent_info(&mut self, snap: usize,id :InputID){
    //     self.parent_snapshot_position = snap;
    //     self.parent_id = id;
    // }
}
