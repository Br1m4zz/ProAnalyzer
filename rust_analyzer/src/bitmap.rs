use crate::fuzz_runner::ExitReason;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum StorageReason{
    Bitmap(BitmapStorageReason),
    // IjonMax(IjonMaxStorageReason),
    Imported,
}

impl StorageReason{
    // 检查存储原因是否仍然有效
    // pub fn still_valid(&self, bitmap: &[u8]) -> bool{
    //     match self{
    //         Self::Bitmap(r) => bitmap[r.index] > r.old, // 如果bitmap对应的偏移的值大于记录的旧值，则有效
    //         // Self::IjonMax(r) => ijon_max[r.index] > r.old,  // 如果ijon_max中的值大于记录的旧值，则有效
    //         Self::Imported => true, // 如果是导入的原因，则有效
    //     }
    // }
    /// 检查是否有新的覆盖率被发现
    pub fn has_new_byte(&self) -> bool {
        match self{
            Self::Bitmap(r) => r.old == 0,  // 如果旧值为0，且有这样的记录，表示有新的代码被触发
            // Self::IjonMax(_r) => true,  // 对于IjonMax类型，始终认为有新的字节被发现
            Self::Imported => false,    // 如果是导入的原因，则没有新的字节被发现
        }
    }
}

///运行的代码覆盖率bitmap的存储原因
/// 
/// 发现新的代码后，记录：覆盖率bitmap偏移、旧值old、新值new
/// 
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BitmapStorageReason {
    pub index: usize,
    pub old: u8,
    pub new: u8,
}

//IJON的bitmap的存储原因
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct IjonMaxStorageReason {
    pub index: usize,
    pub old: u8,
    pub new: u8,
}


//记录测试的四种bitmap
pub struct BitmapHandler {
    normal: Bitmap,
    crash: Bitmap,
    timeout: Bitmap,
    invalid_write_to_payload: Bitmap,
    size: usize,
}

impl BitmapHandler {
    /// 根据传入的size大小，创建一个bitmap的处理句柄，维护四种不同情况的bitmap
    ///
    /// 维护了normal crash timeout invalid_write_to_payload四种不同的bitmap 
    pub fn new(size: usize) -> Self {
        return Self {
            normal: Bitmap::new(size),
            crash: Bitmap::new(size),
            timeout: Bitmap::new(size),
            invalid_write_to_payload: Bitmap::new(size),
            size,
        };
    }

    ///Bitmaphandler的方法，根据目标的退出情况，bitmap的变化情况，返回StorageReason
    pub fn check_new_bytes(
        &mut self,
        run_bitmap: &[u8],
        // ijon_max_map: &[u64],
        etype: &ExitReason,
    ) -> Option<Vec<StorageReason>> {
        match etype {
            ExitReason::Normal(_) => return self.normal.check_new_bytes(run_bitmap),
            ExitReason::Crash(_) => return self.crash.check_new_bytes(run_bitmap),
            ExitReason::Timeout => return self.timeout.check_new_bytes(run_bitmap),
            ExitReason::InvalidWriteToPayload(_) => {
                return self.invalid_write_to_payload.check_new_bytes(run_bitmap)
            }
            _ => return None,
        }
    }

    pub fn size(&self) -> usize {//返回bitmap大小
        self.size
    }
    
    pub fn normal_bitmap(&self) -> &Bitmap{ //返回bitmap的noramal
        return &self.normal
    }
}

#[derive(Clone)]
pub struct Bitmap {
    bits: Vec<u8>,
    // ijon_max: Vec<u64>,
}

impl Bitmap {
    // 使用指定大小创建一个新的Bitmap实例
    pub fn new(size: usize) -> Self {
        // const IJON_MAX_SIZE: usize=256usize; 
        return Self {
            bits: vec![0; size],                // 创建一个长度为size，初始值为0的bits向量
            // ijon_max: vec![0; IJON_MAX_SIZE],   // 创建一个长度为IJON_MAX_SIZE，初始值为0的ijon_max向量
        };
    }
    // 从提供的缓冲区创建一个新的Bitmap实例
    pub fn new_from_buffer(buff: &[u8]) -> Self {
        return Self {
            bits: buff.to_vec(),
            // ijon_max: ijon_buff.to_vec(),
        };
    }

    /// 实际检查运行前后是否有新的覆盖率被发现
    /// 
    /// 输出：发现触发的新代码覆盖率的队列，里面触发新的bitmap部分对应StorageReason
    /// 
    /// 注：新代码覆盖率指从无到有，
    /// 
    pub fn check_new_bytes(&mut self, run_bitmap: &[u8]) -> Option<Vec<StorageReason>> {
        //检查run_bitmap
        assert_eq!(self.bits.len(), run_bitmap.len());// 确保两个数组长度相同，左边是原本记录，右边是新记录
        let mut res = None;
        for (i, (old, new)) in self.bits.iter_mut().zip(run_bitmap.iter()).enumerate() { // 遍历self.bits和run_bitmap的元素
            if *new > *old && *old == 0{    //这里旧值为0，且新值大于旧值：发现新的代码

                 // 如果res是None，则初始化为一个空的Vec
                if res.is_none() { 
                    res = Some(vec![]);
                }
                // 向res中添加一个BitmapStorageReason，记录位置和新旧值
                res.as_mut().unwrap().push(StorageReason::Bitmap(BitmapStorageReason {  
                    index: i,   
                    old: *old,
                    new: *new,
                }));
                *old = *new;    // 更新全局bitmap的旧值为发现的新值
            }
        }
        //检查run_ijon
        // for (i, (old, new)) in self.ijon_max.iter_mut().zip(run_ijon.iter()).enumerate() {
        //     if *new > *old {
        //         if res.is_none() {   // 如果res是None，则初始化为一个空的Vec
        //             res = Some(vec![]);
        //         }
        //         res.as_mut().unwrap().push(StorageReason::IjonMax(IjonMaxStorageReason {    // 向res中添加一个IjonMaxStorageReason，记录位置和新旧值
        //             index: i,
        //             old: *old,
        //             new: *new,
        //         }));
        //         *old = *new;    // 更新旧值为新值
        //     }
        // }
        return res; // 返回结果
    }

    pub fn bits(&self) -> &[u8] {
        return &self.bits;
    }

    // pub fn ijon_max_vals(&self) -> &[u64] {
    //     return &self.ijon_max;
    // }
}
