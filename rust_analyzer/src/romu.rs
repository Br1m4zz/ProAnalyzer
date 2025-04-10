//随机数生成器（PRNG），使用Romulus算法
pub struct RomuPrng {
    xstate: u64,    
    ystate: u64,    
}

impl RomuPrng {
     // 创建一个新的RomuPrng实例，初始化内部状态
    pub fn new(xstate: u64, ystate: u64) -> Self {
        return Self { xstate, ystate };
    }

    /*
    pub fn range(&mut self, min: usize, max: usize) -> usize {
        return ((self.next_u64() as usize) % (max - min)) + min;
    }
    */
    // 使用单个u64种子创建一个新的RomuPrng实例
    pub fn new_from_u64(seed: u64) -> Self {
        return Self::new(seed, seed ^ 0xec77152282650854);
    }

    /* 
    pub fn next_u32(&mut self) -> u32 {
        self.next_u64() as u32
    }
    */
    // 生成下一个u64类型的伪随机数
    pub fn next_u64(&mut self) -> u64 {
        let xp = self.xstate;   // 保存当前xstate
        self.xstate = 15241094284759029579u64.wrapping_mul(self.ystate);    // 更新xstate为当前ystate乘以一个固定的大数，使用wrapping_mul防止溢出
        self.ystate = self.ystate.wrapping_sub(xp); // 更新ystate为当前ystate减去保存的xstate，使用wrapping_sub防止溢出
        self.ystate = self.ystate.rotate_left(27);
        return xp;  // 返回更新前的xstate作为随机数
    }
}
