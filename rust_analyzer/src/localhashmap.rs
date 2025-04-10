use std::collections::HashMap;
use crate::hash;

// const LOCALBITMAPSIZE: usize = 1 << 26;
pub struct LocalHashmap {
    run_bitmap_seen: HashMap<u64, usize>, // 记录 run_bitmap 的哈希值和序号
    ijon_map_seen: HashMap<u64, usize>,  // 记录 ijon_map 的哈希值和序号
    cov_bitmap_seen: HashMap<u64, usize>, // 记录 cov_bitmap 的哈希值和序号
    run_bitmap_current_index: usize,                // 全局递增序号
    cov_bitmap_current_index: usize,                // 全局递增序号
    ijon_bitmap_current_index: usize,      // ijon_map 的全局递增序号
}

impl Default for LocalHashmap {
    fn default() -> Self {
        Self {
            run_bitmap_seen: HashMap::new(),
            cov_bitmap_seen: HashMap::new(),
            ijon_map_seen: HashMap::new(),
            run_bitmap_current_index: 0,
            cov_bitmap_current_index: 0,
            ijon_bitmap_current_index: 0,
        }
    }
}

impl LocalHashmap {
    pub fn new() -> Self {
        Self::default()
    }

    /// 检查 run_bitmap 和 ijon_map 是否已存在于哈希表，如果不存在，则记录序号
    pub fn handle_run_bitmap(&mut self, run_bitmap: &[u8]) -> usize {
        let cur_exec_hash = hash::hash64(run_bitmap, run_bitmap.len());

        // 检查是否已存在 run_bitmap 的哈希值
        if let Some(&run_index) = self.run_bitmap_seen.get(&cur_exec_hash) {
            return run_index; // 返回已存在的序号
        }

        // 如果不存在，则记录新条目并分配新序号
        let new_index = self.run_bitmap_current_index;
        self.run_bitmap_current_index += 1; // 更新全局序号
        self.run_bitmap_seen.insert(cur_exec_hash, new_index);

        new_index
    }

    /// 处理 ijon_map 的哈希值并返回对应序号
    pub fn handle_ijon_map(&mut self, ijon_map: &[u8]) -> usize {
        let sv_hash = hash::hash64(ijon_map, ijon_map.len());

        // 检查是否已存在 ijon_map 的哈希值
        if let Some(&ijon_index) = self.ijon_map_seen.get(&sv_hash) {
            return ijon_index; // 返回已存在的序号
        }

        // 如果不存在，则记录新条目并分配新序号
        let new_index = self.ijon_bitmap_current_index;
        self.ijon_bitmap_current_index += 1; // 更新全局序号
        self.ijon_map_seen.insert(sv_hash, new_index);

        new_index
    }


    /// 处理 cov_bitmap：
    /// 将传入的 run_bitmap 中所有非 0 的值转换为 1，
    /// 计算 cov_bitmap 的哈希值，并检查是否已经存在于 cov_bitmap_seen 中，
    /// 如果不存在则记录新序号
    pub fn handle_cov_bitmap(&mut self, run_bitmap: &[u8]) -> usize {
        // 生成 cov_bitmap，将 run_bitmap 中所有非 0 的值统一转换为 1
        let cov_bitmap: Vec<u8> = run_bitmap.iter().map(|&x| if x > 0 { 1 } else { 0 }).collect();

        // 计算 cov_bitmap 的哈希值
        let cov_hash = hash::hash64(&cov_bitmap, cov_bitmap.len());

        // 检查 cov_hash 是否已经存在
        if let Some(&existing_index) = self.cov_bitmap_seen.get(&cov_hash) {
            return existing_index;
        }

        // 不存在，则记录新条目，并分配新序号
        let new_index = self.cov_bitmap_current_index;
        self.cov_bitmap_current_index += 1;
        self.cov_bitmap_seen.insert(cov_hash, new_index);

        new_index
    }
    /// 清空所有记录
    pub fn clear(&mut self) {
        self.run_bitmap_seen.clear();
        self.cov_bitmap_seen.clear();
        self.ijon_map_seen.clear();
        self.run_bitmap_current_index = 0;
        self.cov_bitmap_current_index = 0;
        self.ijon_bitmap_current_index = 0;
    }
}
