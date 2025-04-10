use std::collections::HashMap;

use crate::data_buff::DataBuff;
use crate::random::distributions::Distributions;

#[derive(Clone)]
pub enum DictEntry{
    Replace(Vec<u8>, Vec<u8>)
}

#[derive(Clone)]
pub struct CustomDict{
    groups: Vec<Vec<DictEntry>>,
    lhs_to_groups: HashMap<Vec<u8>,Vec<Vec<u8>>>,
}

impl CustomDict{
    pub fn new() -> Self{
        return Self{groups: vec!(), lhs_to_groups: HashMap::new()}
    }

    pub fn new_from_groups(groups: Vec<Vec<DictEntry>>) -> Self{
        let mut lhs_to_groups = HashMap::new();
        for group in groups.iter() {
            for entry in group.iter() {
                match entry {
                    DictEntry::Replace(lhs, rhs) => {
                        let entry = lhs_to_groups.entry(lhs.clone()).or_insert_with(|| vec!());
                        entry.push(rhs.clone());
                    }
                }
            }
        }
        return Self{groups, lhs_to_groups}
    }

    pub fn len(&self) -> usize {
        return self.groups.len();
    }

    ///基于变异字典的变异
    /// 
    /// 返回是否成功完成了变异
    /// 
    /// 
    pub fn mutate(&self, buff: &mut DataBuff, dist:  &Distributions) -> bool {
        //尝试寻找缓冲区LHS中与字典相同的内容RHS，做替换处理
        if let Some(rhs) = self.sample_rhs(buff, dist) {
            if dist.gen(){  
                //将rhs拷贝到buff起点
                buff.copy_from(&rhs, 0);
                return dist.gen::<bool>();
            }
        }
        //尝试从变异字典中获取一个条目（entry）。如果成功获取到 entry，则继续
        if let Some(entry) = self.sample_entry(buff,dist){
            match entry{
                //匹配获取到的 entry。如果是 DictEntry::Replace(lhs, rhs) 类型的条目，执行下一步。
                DictEntry::Replace(lhs, rhs) => {
                    //使用 find_pos 函数在 buff 中查找左侧值（lhs）的位置。如果找到，使用 buff.copy_from 方法将 rhs 复制到找到的位置（pos）
                    if let Some(pos) = self.find_pos(buff, &lhs, dist){
                        buff.copy_from(&rhs, pos);
                    }
                    return dist.gen::<bool>();
                }
            }
        }
        return true;
    }

    /// 根据输入缓冲区的内容（左侧值，lhs）来选择一个合适的替换值（右侧值，rhs），以生成新的测试用例。
    /// 
    /// 如果字典中没有对应的左侧值，函数将返回 None，表示没有可用的替换值
    /// 
    pub fn sample_rhs(&self, buff: &DataBuff, dist: &Distributions) -> Option<&Vec<u8>> {
        if self.lhs_to_groups.contains_key(buff.as_slice()) {
            let opts = &self.lhs_to_groups[buff.as_slice()];
            assert!(opts.len() > 0);
            return Some(&opts[dist.gen_range(0, opts.len())]);
        }
        return None
    }

    ///
    /// 从一组预定义的字典条目中随机选择一个，以便对测试数据进行变异，生成新的测试用例。
    /// 
    /// 
    pub fn sample_entry(&self, _buff: &DataBuff, dist: &Distributions) -> Option<&DictEntry> {
        if self.groups.len() == 0 {return None}
        //随机选择
        let group = &self.groups[dist.gen_range(0,self.groups.len())];
        return Some(&group[dist.gen_range(0,group.len())]);
    }

    pub fn find_pos(&self, buff: &DataBuff, lhs: &[u8], dist: &Distributions) -> Option<usize>{
        let mut offsets = vec!();
        for (i,win) in buff.as_slice().windows(lhs.len()).enumerate() {
            if win == lhs {
                offsets.push(i);
            }
        }
        if offsets.len() > 0 {
            return Some(offsets[dist.gen_range(0,offsets.len())]);
        }
        if buff.len() >= lhs.len(){
            return Some(dist.gen_range(0,buff.len()-lhs.len()+1));
        }
        return None
    }
}

