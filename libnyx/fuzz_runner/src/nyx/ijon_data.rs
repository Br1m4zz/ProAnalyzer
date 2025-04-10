#[derive(Debug, Copy, Clone)]
#[repr(C, packed(1))]
pub struct InterpreterData{
    pub executed_opcode_num: u32 // 执行的操作码数量
}

#[derive(Copy, Clone)]
#[repr(C)]
pub struct IjonData {
    pub max_data: [u8;2048],  // Ijon使用的数据数组
}

#[derive(Copy, Clone)]
#[repr(C, packed(1))]
pub struct SharedFeedbackData{//共享内存保留的数据
    pub interpreter: InterpreterData,
    pad: [u8; 0x1000/2-std::mem::size_of::<InterpreterData>()],
    pub ijon: IjonData,
}

pub struct FeedbackBuffer {
    pub shared: &'static mut SharedFeedbackData,
}

impl FeedbackBuffer{
    pub fn new(shared: &'static mut SharedFeedbackData) -> Self{
        Self{shared}
    }
}