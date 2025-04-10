use std::i64;
use std::ops::Range;
//use std::rc::Rc;

use crate::data_buff::DataBuff;
use crate::custom_dict::CustomDict;
use crate::primitive_mutator::inplace_mutation::{InplaceMutation, InplaceMutationType};
use crate::primitive_mutator::size_changing_mutation::{
    SizeChangingMutation, SizeChangingMutationType,
};
use crate::random::distributions::Distributions;

const INTERESTING_U8: [u8; 9] = [(-128i8) as u8, (-1i8) as u8, 0, 1, 16, 32, 64, 100, 127];
const INTERESTING_U16: [u16; 19] = [
    (-128i16) as u16,
    (-1i16) as u16,
    0,
    1,
    16,
    32,
    64,
    100,
    127, //u8
    (-32768i16) as u16,
    (-129i16) as u16,
    128,
    255,
    256,
    512,
    1000,
    1024,
    4096,
    32767,
];
const INTERESTING_U32: [u32; 27] = [
    (-128i32) as u32,
    (-1i32) as u32,
    0,
    1,
    16,
    32,
    64,
    100,
    127, //u8
    (-32768i32) as u32,
    (-129i32) as u32,
    128,
    255,
    256,
    512,
    1000,
    1024,
    4096,
    32767, //u16
    (-2147483648i32) as u32,
    (-100663046i32) as u32,
    (-32769i32) as u32,
    32768,
    65535,
    65536,
    100663045,
    2147483647,
];
const INTERESTING_U64: [u64; 30] = [
    (-128i64) as u64,
    (-1i64) as u64,
    0,
    1,
    16,
    32,
    64,
    100,
    127, //u8
    (-32768i32) as u64,
    (-129i64) as u64,
    128,
    255,
    256,
    512,
    1000,
    1024,
    4096,
    32767, //u16
    (-2147483648i64) as u64,
    (-100663046i64) as u64,
    (-32769i64) as u64,
    32768,
    65535,
    65536,
    100663045,
    2147483647, //u32
    i64::MIN as u64,
    0x7fffffffffffffff,
    0x8080808080808080,
];

#[derive(Clone, Eq, Hash, PartialEq, Debug)]
pub enum Mutation {
    Inplace(InplaceMutation),
    SizeChanging(SizeChangingMutation),
}

impl Mutation {
    pub fn apply(&self, buff: &mut DataBuff) {
        match self {
            Mutation::Inplace(x) => x.apply(buff),
            Mutation::SizeChanging(x) => x.apply(buff),
        }
    }
}

/// 数据变异器
/// 
/// 当变异策略是变异节点中的数据，则会用上他
/// 
pub struct PrimitiveMutator {}

impl PrimitiveMutator {

    ///原语变异器PrimitiveMutator
    pub fn new() -> Self {
        return Self { };
    }

    ///执行变异
    /// 
    /// 目前数据变异的策略有：
    /// 
    /// 1. 字典变异（dict.mutate）
    /// 
    /// 2. 数据buffer的内容替换变异（gen_inplace_mutation）
    /// 
    pub fn mutate(&self, buff: &mut DataBuff, dict: Option<&CustomDict>, dist: &Distributions) {
        // buffer是空的则有问题
        if buff.is_empty() {
            return;
        }

        // 有变异字典，优先调用字典变异
        if let Some(dict) = dict{
            if  dist.should_mutate_dict() {
                //调用字典变异
                let continue_mutation = dict.mutate(buff, dist);
                //字典变异成功则停止
                if ! continue_mutation {return}
            }
        }
        //TODO add size changing mutations if buff.available != 0
        
        
        //进行原数据的inplace_mutation（AFL的位变化变异）
        let mutation = self.gen_inplace_mutation(buff,dist);
        mutation.apply(buff);//根据确定的变异参数执行变异
    }

    /// gen_inplace_mutation的helper函数
    fn gen_inplace_mutation_type(&self, buff: &DataBuff,dist: &Distributions) -> InplaceMutationType {
        assert!(!buff.is_empty());
        for _ in 1..5 {
            let t = *dist.gen_inplace_mutation_type();
            if t.min_size() <= buff.len() {
                return t;
            }
        }
        return InplaceMutationType::FlipBitT;
    }

    ///
    /// 类似AFL的位变化变异
    /// 实现了一系列原位变异操作，用于确定模糊测试中对数据缓冲区 buff 进行数值替换变异的参数：offset、val、flip_endian等。
    /// 它根据分布 dist 生成的变异类型 m_type 来决定执行哪种变异
    /// 
    pub fn gen_inplace_mutation(&self, buff: &DataBuff, dist: &Distributions) -> InplaceMutation {
        use InplaceMutation::*;
        use InplaceMutationType::*;
        let m_type = self.gen_inplace_mutation_type(buff,dist);
        match m_type {
            FlipBitT => FlipBit {
                offset: self.gen_offset(1, buff, dist),
                bit: dist.gen_range(0, 8),
            },
            AddU8T => AddU8 {
                offset: self.gen_offset(1, buff, dist),
                val: self.gen_arith_val(dist) as u8,
            },
            AddU16T => AddU16 {
                offset: self.gen_offset(2, buff, dist),
                val: self.gen_arith_val(dist) as u16,
                flip_endian: dist.gen_endianess(),
            },
            AddU32T => AddU32 {
                offset: self.gen_offset(4, buff, dist),
                val: self.gen_arith_val(dist) as u32,
                flip_endian: dist.gen_endianess(),
            },
            AddU64T => AddU64 {
                offset: self.gen_offset(8, buff, dist),
                val: self.gen_arith_val(dist) as u64,
                flip_endian: dist.gen_endianess(),
            },
            InterestingU8T => InterestingU8 {
                offset: self.gen_offset(1, buff, dist),
                val: self.gen_pick(&INTERESTING_U8, dist),
            },
            InterestingU16T => InterestingU16 {
                offset: self.gen_offset(2, buff, dist),
                val: self.gen_pick(&INTERESTING_U16, dist),
                flip_endian: dist.gen_endianess(),
            },
            InterestingU32T => InterestingU32 {
                offset: self.gen_offset(4, buff, dist),
                val: self.gen_pick(&INTERESTING_U32, dist),
                flip_endian: dist.gen_endianess(),
            },
            InterestingU64T => InterestingU64 {
                offset: self.gen_offset(8, buff, dist),
                val: self.gen_pick(&INTERESTING_U64, dist),
                flip_endian: dist.gen_endianess(),
            },
            OverwriteRandomByteT => {
                let offset = self.gen_offset(1, buff, dist);
                OverwriteRandomByte {
                    offset,
                    val: dist.gen_range(1, 0xff) ^ buff.read_u8(offset),
                }
            }
            OverwriteChunkT => {
                let src = self.gen_block_range(buff, dist);
                let len = src.end - src.start;
                let mut dst = self.gen_offset(len, buff, dist);
                while src.start == dst && len != buff.len() {
                    dst = self.gen_offset(len, buff, dist);
                }
                OverwriteChunk { src, dst }
            }
            OverwriteRandomT => {
                let mut dst = self.gen_block_range(buff, dist);
                let data = dist.gen_random_overwrite_data(&dst);
                dst.end = dst.start+data.len();
                OverwriteRandom {
                    data,
                    dst: dst.start,
                }
            }
            OverwriteFixedT => OverwriteFixed {
                block: self.gen_block_range(buff, dist),
                val: dist.gen(),
            },
        }
    }

    // //使用
    // pub fn gen_fixed_value_mutation_at_offset(&self, buff: &DataBuff, offset: usize) -> InplaceMutation {
    //     assert!(offset < buff.len());
    //     use InplaceMutation::*;
    //     // 生成InplaceMutation对象，表示在指定偏移位置写入0xFF
    //     OverwriteFixed {
    //         block: offset..offset + 1,
    //         val: 0xff,
    //     }
    // }


    fn gen_size_changing_mutation_type(&self, buff: &DataBuff,dist: &Distributions) -> SizeChangingMutationType {
        assert!(buff.capacity() > 16);
        assert!(!buff.is_empty());
        for _ in 1..5 {
            let t = *dist.gen_size_changing_mutation_type();
            if t.min_size() <= buff.len() && t.min_available() < buff.available() {
                return t;
            }
        }
        if buff.available() > SizeChangingMutationType::InsertRandomT.min_available() {
            return SizeChangingMutationType::InsertRandomT;
        }
        return SizeChangingMutationType::DeleteT;
    }

    ///改变大小的变异操作，用于模糊测试中对数据缓冲区 buff 进行变异。它根据分布 dist 生成的变异类型 m_type 来决定执行哪种变异
    /// 
    /// 
    pub fn gen_size_changing_mutation(&self, buff: &DataBuff,dist: &Distributions) -> SizeChangingMutation {
        use SizeChangingMutation::*;
        use SizeChangingMutationType::*;
        let m_type = self.gen_size_changing_mutation_type(buff, dist);
        match m_type {
            DeleteT => Delete {
                block: self.gen_block_range(buff, dist),
            },
            InsertChunkT => InsertChunk {
                src: self.gen_insert_block_range(buff, dist),
                dst: self.gen_offset(1, buff, dist),
            },
            InsertFixedT => InsertFixed {
                dst: self.gen_offset(1, buff, dist),
                amount: dist.gen_range(1, buff.capacity() - buff.len()),
                val: dist.gen(),
            },
            InsertRandomT => {
                let max = dist.gen_range(1, buff.available());
                InsertRandom {
                    data: (0..max).map(|_| dist.gen()).collect(),
                    dst: self.gen_offset(1, buff, dist),
                }
            }
        }
    }

    /// 这个对数据buffer的变异函数还没被用
    pub fn gen_mutation(&self, buff: &DataBuff,dist: &Distributions) -> Mutation {
        //根据分布情况确定是否使用Inplace变异
        if dist.gen() {
            return Mutation::Inplace(self.gen_inplace_mutation(buff, dist));
        }
        //不用Inplace变异则进行大小变化变异
        return Mutation::SizeChanging(self.gen_size_changing_mutation(buff, dist));
    }

    // pub fn calibrate_fix_value_ff_at(&self, buff: &DataBuff,offset:usize) -> Mutation {
    //     return Mutation::Inplace(self.gen_fixed_value_mutation_at_offset(buff,offset));
    // }

    fn gen_offset(&self, size: usize, buff: &DataBuff,dist: &Distributions) -> usize {
        assert!(buff.len() >= size);
        dist.gen_range(0, buff.len() - size + 1)
    }

    fn gen_arith_val(&self,dist: &Distributions) -> i8 {
        if dist.gen() {
            dist.gen_range(1, 35)
        } else {
            dist.gen_range(-35, -1)
        }
    }

    fn gen_pick<T: Copy>(&self, data: &[T],dist: &Distributions) -> T {
        data[dist.gen_range(0, data.len())]
    }
    fn gen_insert_block_range(&self, buff: &DataBuff,dist: &Distributions) -> Range<usize> {
        return self.gen_block_max(0..buff.len(), buff.capacity() - buff.len(), dist);
    }
    fn gen_block_range(&self, buff: &DataBuff,dist: &Distributions) -> Range<usize> {
        return self.gen_block_max(0..buff.len(), buff.len(), dist);
    }
    fn gen_block_max(&self, range: Range<usize>, max_len: usize,dist: &Distributions) -> Range<usize> {
        if range.start == range.end {
            return range;
        }
        let (mut min, mut max) = *dist.gen_block_size();
        if max > max_len {
            max = max_len;
        }
        let len = range.end - range.start;
        if max > len {
            max = len;
        }
        if min >= max {
            min = 1;
        }
        let size = dist.gen_range(min, max + 1);
        let start = dist.gen_range(0, range.end - size + 1);
        return start..start + size;
    }

    pub fn gen_num_array_elems(
        &self,
        elem_size: usize,
        min_elems: usize,
        max_elems: usize,
        max_data: usize,
        dist: &Distributions
    ) -> usize {
        if max_data / elem_size > min_elems {
            return dist
                .gen_range(min_elems, max_elems.min(max_data / elem_size));
        }
        return max_data / elem_size;
    }
}


// pub enum MutationDefinite {
//     /// 全比特翻转：in_data[offset] = in_data[offset] ^ 0xff
//     FullBitFlip(InplaceMutation),
//     /// 最低位翻转：in_data[offset] = in_data[offset] ^ 0xfe
//     LowestBitFlip(InplaceMutation),
//     /// 数值减法扰动：in_data[offset] = (in_data[offset] ^ 0x01).wrapping_sub(0x10)
//     Subtraction(InplaceMutation),
//     /// 数值加法扰动：in_data[offset] = in_data[offset].wrapping_add(0x20)
//     Addition(InplaceMutation),
//     /// 长度字段识别（2字节分析）：对读取的 u16 值进行判断，若 plausibly 为长度字段则置零，否则按位取反
//     LengthFieldU16(InplaceMutation),
//     /// 校验和字段识别：对连续两个字节计算差值，差值过大则全比特取反，否则仅翻转最低位
//     ChecksumField(InplaceMutation),
// }

// impl MutationDefinite {
//     /// 对应当前变异算子，在给定缓冲区上执行变异
//     pub fn apply(&self, buff: &mut DataBuff) {
//         match self {
//             MutationDefinite::FullBitFlip(m) => m.apply(buff),
//             MutationDefinite::LowestBitFlip(m) => m.apply(buff),
//             MutationDefinite::Subtraction(m) => m.apply(buff),
//             MutationDefinite::Addition(m) => m.apply(buff),
//             MutationDefinite::LengthFieldU16(m) => m.apply(buff),
//             MutationDefinite::ChecksumField(m) => m.apply(buff),
//         }
//     }
// }

pub struct PrimitiveMutatorDefenite {}
impl PrimitiveMutatorDefenite {
    pub fn new() -> Self {
        return Self { };
    }
    
    /// 1. 全比特翻转检测
    /// 操作：in_data[i] = in_data[i] ^ 0xff
    /// 用于检测程序对极端比特变化的敏感性
    pub fn gen_full_bit_flip_at_offset(
        &self,
        buff: &DataBuff,
        offset: usize,
    ) -> InplaceMutation {
        assert!(
            offset < buff.len(),
            "Offset {} out of bounds (buffer length {})",
            offset,
            buff.len()
        );
        let orig = buff.read_u8(offset);
        let new_val = orig ^ 0xff;
        InplaceMutation::OverwriteFixed {
            block: offset..(offset + 1),
            val: new_val,
        }
    }

    /// 2. 最低位翻转检测
    /// 操作：首先全比特反转，再额外反转最低位，即 in_data[i] = in_data[i] ^ 0xfe
    /// 用于检测对最低位变化（如奇偶校验位）的敏感度
    pub fn gen_lowest_bit_flip_at_offset(
        &self,
        buff: &DataBuff,
        offset: usize,
    ) -> InplaceMutation {
        assert!(
            offset < buff.len(),
            "Offset {} out of bounds (buffer length {})",
            offset,
            buff.len()
        );
        let orig = buff.read_u8(offset);
        let new_val = orig ^ 0x01;
        InplaceMutation::OverwriteFixed {
            block: offset..(offset + 1),
            val: new_val,
        }
    }

    /// 3. 数值减法扰动
    /// 操作：先翻转最低位，再减 0x10，即 in_data[i] = (in_data[i] ^ 0x01) - 0x10
    /// 用于测试程序对数值减少（如长度字段、计数器）的敏感度
    pub fn gen_subtraction_at_offset(
        &self,
        buff: &DataBuff,
        offset: usize,
    ) -> InplaceMutation {
        assert!(
            offset < buff.len(),
            "Offset {} out of bounds (buffer length {})",
            offset,
            buff.len()
        );
        let orig = buff.read_u8(offset);
        let new_val = (orig).wrapping_sub(0x10);
        InplaceMutation::OverwriteFixed {
            block: offset..(offset + 1),
            val: new_val,
        }
    }

    /// 4. 数值加法扰动
    /// 操作：in_data[i] = in_data[i] + 0x20
    /// 用于测试程序对数值增加（如索引或偏移量）的敏感性
    pub fn gen_addition_at_offset(
        &self,
        buff: &DataBuff,
        offset: usize,
    ) -> InplaceMutation {
        assert!(
            offset < buff.len(),
            "Offset {} out of bounds (buffer length {})",
            offset,
            buff.len()
        );
        let orig = buff.read_u8(offset);
        let new_val = orig.wrapping_add(0x10);
        InplaceMutation::OverwriteFixed {
            block: offset..(offset + 1),
            val: new_val,
        }
    }

}









#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_inplace() {
        let dist = crate::random::distributions::Distributions::new(vec!());
        let mutator = PrimitiveMutator::new();
        let mut data = vec![0u8; 1024];
        let mut buff = DataBuff::new(&mut data, 0);

        for _ in 0..1000 {
            let len = dist.gen_range(1, 1024);
            buff.set_to_random(len, &dist);
            let mutation = mutator.gen_inplace_mutation(&buff,&dist);

            mutation.apply(&mut buff);
            assert_eq!(buff.len(), len);
        }
    }

    #[cfg(test)]
    #[test]
    fn test_sized() {
        let dist = crate::random::distributions::Distributions::new(vec!());
        let mutator = PrimitiveMutator::new();
        let mut data = vec![0u8; 1024];
        let mut buff = DataBuff::new(&mut data, 0);

        for _ in 0..1000 {
            let len = dist.gen_range(1, 1024);
            buff.set_to_random(len, &dist);
            let mutation = mutator.gen_size_changing_mutation(&buff, &dist);
            println!("{:?} on buff of length {}", mutation, buff.len());
            mutation.apply(&mut buff);
        }
    }

    use std::collections::HashMap;
    #[test]
    fn test_uniqueness() {
        let dist = crate::random::distributions::Distributions::new(vec!());
        let mutator = PrimitiveMutator::new();
        let mut data = vec![0u8; 1024];
        let mut buff = DataBuff::new(&mut data, 0);
        let mut storage = HashMap::<Vec<u8>, usize>::new();
        let mut infos = HashMap::<Vec<u8>, HashMap<Mutation, usize>>::new();
        let base = (0..256)
            .map(|_| dist.gen::<u8>())
            .collect::<Vec<_>>();
        let iters = 10000;
        for _ in 0..iters {
            buff.set_to_slice(&base);
            let mutation = mutator.gen_mutation(&buff, &dist);
            mutation.apply(&mut buff);
            let inf = infos
                .entry(buff.as_slice().to_vec())
                .or_insert_with(|| HashMap::new());
            *(inf.entry(mutation).or_insert(0)) += 1;
            *storage.entry(buff.as_slice().to_vec()).or_insert(0) += 1;
        }
        println!("{} of {} iters produced uniq results", storage.len(), iters);
        let mut generated = storage.keys().collect::<Vec<_>>();
        generated.sort_unstable_by_key(|x| std::usize::MAX - storage.get(*x).unwrap());
        for dat in generated.iter().take(20) {
            println!(
                "data was hit {:?} times, produced by {:?}",
                storage.get(*dat),
                infos.get(*dat)
            );
        }
        assert!(storage.len() >= iters - (iters / 5)); //max 5% of the mutation should be duplicates
    }

    /// 测试全比特翻转算子：in_data[offset] = in_data[offset] ^ 0xff
    #[test]
    fn test_full_bit_flip_random() {
        let dist = Distributions::new(vec!());
        let mutator = PrimitiveMutatorDefenite::new();
        let mut data = vec![0u8; 1024];
        let mut buff = DataBuff::new(&mut data, 0);

        for _ in 0..1000 {
            let len = dist.gen_range(1, 1024);
            buff.set_to_random(len, &dist);
            let off = dist.gen_range(0, len);
            let orig = buff.read_u8(off);
            let expected = orig ^ 0xff;

            let mutation = mutator.gen_full_bit_flip_at_offset(&buff, off);
            mutation.apply(&mut buff);
            let result = buff.read_u8(off);

            assert_eq!(result, expected, "Full bit flip failed at offset {}: expected 0x{:02X}, got 0x{:02X}", off, expected, result);
            assert_eq!(buff.len(), len, "Buffer length changed after full bit flip");
        }
    }

    /// 测试最低位翻转算子：in_data[offset] = in_data[offset] ^ 0xfe
    #[test]
    fn test_lowest_bit_flip_random() {
        let dist = Distributions::new(vec!());
        let mutator = PrimitiveMutatorDefenite::new();
        let mut data = vec![0u8; 1024];
        let mut buff = DataBuff::new(&mut data, 0);

        for _ in 0..1000 {
            let len = dist.gen_range(1, 1024);
            buff.set_to_random(len, &dist);
            let off = dist.gen_range(0, len);
            let orig = buff.read_u8(off);
            let expected = orig ^ 0xfe;

            let mutation = mutator.gen_lowest_bit_flip_at_offset(&buff, off);
            mutation.apply(&mut buff);
            let result = buff.read_u8(off);

            assert_eq!(result, expected, "Lowest bit flip failed at offset {}: expected 0x{:02X}, got 0x{:02X}", off, expected, result);
            assert_eq!(buff.len(), len, "Buffer length changed after lowest bit flip");
        }
    }

    /// 测试数值减法扰动算子：
    /// in_data[offset] = (in_data[offset] ^ 0x01).wrapping_sub(0x10)
    #[test]
    fn test_subtraction_mutation_random() {
        let dist = Distributions::new(vec!());
        let mutator = PrimitiveMutatorDefenite::new();
        let mut data = vec![0u8; 1024];
        let mut buff = DataBuff::new(&mut data, 0);

        for _ in 0..1000 {
            let len = dist.gen_range(1, 1024);
            buff.set_to_random(len, &dist);
            let off = dist.gen_range(0, len);
            let orig = buff.read_u8(off);
            let expected = (orig ^ 0x01).wrapping_sub(0x10);

            let mutation = mutator.gen_subtraction_at_offset(&buff, off);
            mutation.apply(&mut buff);
            let result = buff.read_u8(off);

            assert_eq!(result, expected, "Subtraction mutation failed at offset {}: from 0x{:02X} expected 0x{:02X}, got 0x{:02X}", off, orig, expected, result);
            assert_eq!(buff.len(), len, "Buffer length changed after subtraction mutation");
        }
    }

    /// 测试数值加法扰动算子：
    /// in_data[offset] = in_data[offset].wrapping_add(0x20)
    #[test]
    fn test_addition_mutation_random() {
        let dist = Distributions::new(vec!());
        let mutator = PrimitiveMutatorDefenite::new();
        let mut data = vec![0u8; 1024];
        let mut buff = DataBuff::new(&mut data, 0);
        
        for _ in 0..1000 {
            let len = dist.gen_range(1, 1024);
            buff.set_to_random(len, &dist);
            let off = dist.gen_range(0, len);
            let orig = buff.read_u8(off);
            let expected = orig.wrapping_add(0x20);

            let mutation = mutator.gen_addition_at_offset(&buff, off);
            mutation.apply(&mut buff);
            let result = buff.read_u8(off);

            assert_eq!(result, expected, "Addition mutation failed at offset {}: from 0x{:02X} expected 0x{:02X}, got 0x{:02X}", off, orig, expected, result);
            assert_eq!(buff.len(), len, "Buffer length changed after addition mutation");
        }
    }


    

    //  测试 MutationDefinite::FullBitFlip 随机 500 次
    //  操作：in_data[offset] = in_data[offset] ^ 0xff
    // #[test]
    // fn test_mutation_definite_full_bit_flip_random() {
    //     let dist = Distributions::new(vec!());
    //     let mut mutator = PrimitiveMutatorDefenite::new();
    //     let mut data = vec![0u8; 1024];
    //     let mut buff = DataBuff::new(&mut data, 0);

    //     for _ in 0..500 {
    //         let len = dist.gen_range(1, 1024);
    //         buff.set_to_random(len, &dist);
    //         let off = dist.gen_range(0, len);
    //         let orig = buff.read_u8(off);
    //         let expected = orig ^ 0xff;

    //         let mutation = mutator.gen_full_bit_flip_at_offset(&buff, off);
    //         let definite = MutationDefinite::FullBitFlip(mutation);
    //         definite.apply(&mut buff);

    //         let result = buff.read_u8(off);
    //         assert_eq!(
    //             result, expected,
    //             "FullBitFlip failed at offset {}: expected 0x{:02X}, got 0x{:02X}",
    //             off, expected, result
    //         );
    //         assert_eq!(
    //             buff.len(),
    //             len,
    //             "Buffer length changed after FullBitFlip"
    //         );
    //     }
    // }

    // /// 测试 MutationDefinite::LowestBitFlip 随机 500 次
    // /// 操作：in_data[offset] = in_data[offset] ^ 0xfe
    // #[test]
    // fn test_mutation_definite_lowest_bit_flip_random() {
    //     let dist = Distributions::new(vec!());
    //     let mut mutator = PrimitiveMutatorDefenite::new();
    //     let mut data = vec![0u8; 1024];
    //     let mut buff = DataBuff::new(&mut data, 0);

    //     for _ in 0..500 {
    //         let len = dist.gen_range(1, 1024);
    //         buff.set_to_random(len, &dist);
    //         let off = dist.gen_range(0, len);
    //         let orig = buff.read_u8(off);
    //         let expected = orig ^ 0xfe;

    //         let mutation = mutator.gen_lowest_bit_flip_at_offset(&buff, off);
    //         let definite = MutationDefinite::LowestBitFlip(mutation);
    //         definite.apply(&mut buff);

    //         let result = buff.read_u8(off);
    //         assert_eq!(
    //             result, expected,
    //             "LowestBitFlip failed at offset {}: expected 0x{:02X}, got 0x{:02X}",
    //             off, expected, result
    //         );
    //         assert_eq!(
    //             buff.len(),
    //             len,
    //             "Buffer length changed after LowestBitFlip"
    //         );
    //     }
    // }

    // /// 测试 MutationDefinite::Subtraction 随机 500 次
    // /// 操作：in_data[offset] = (in_data[offset] ^ 0x01).wrapping_sub(0x10)
    // #[test]
    // fn test_mutation_definite_subtraction_random() {
    //     let dist = Distributions::new(vec!());
    //     let mut mutator = PrimitiveMutatorDefenite::new();
    //     let mut data = vec![0u8; 1024];
    //     let mut buff = DataBuff::new(&mut data, 0);

    //     for _ in 0..500 {
    //         let len = dist.gen_range(1, 1024);
    //         buff.set_to_random(len, &dist);
    //         let off = dist.gen_range(0, len);
    //         let orig = buff.read_u8(off);
    //         let expected = (orig ^ 0x01).wrapping_sub(0x10);

    //         let mutation = mutator.gen_subtraction_at_offset(&buff, off);
    //         let definite = MutationDefinite::Subtraction(mutation);
    //         definite.apply(&mut buff);

    //         let result = buff.read_u8(off);
    //         assert_eq!(
    //             result, expected,
    //             "Subtraction mutation failed at offset {}: from 0x{:02X} expected 0x{:02X}, got 0x{:02X}",
    //             off, orig, expected, result
    //         );
    //         assert_eq!(
    //             buff.len(),
    //             len,
    //             "Buffer length changed after Subtraction mutation"
    //         );
    //     }
    // }

    // /// 测试 MutationDefinite::Addition 随机 500 次
    // /// 操作：in_data[offset] = in_data[offset].wrapping_add(0x20)
    // #[test]
    // fn test_mutation_definite_addition_random() {
    //     let dist = Distributions::new(vec!());
    //     let mut mutator = PrimitiveMutatorDefenite::new();
    //     let mut data = vec![0u8; 1024];
    //     let mut buff = DataBuff::new(&mut data, 0);
        
    //     for _ in 0..500 {
    //         let len = dist.gen_range(1, 1024);
    //         buff.set_to_random(len, &dist);
    //         let off = dist.gen_range(0, len);
    //         let orig = buff.read_u8(off);
    //         let expected = orig.wrapping_add(0x20);

    //         let mutation = mutator.gen_addition_at_offset(&buff, off);
    //         let definite = MutationDefinite::Addition(mutation);
    //         definite.apply(&mut buff);

    //         let result = buff.read_u8(off);
    //         assert_eq!(
    //             result, expected,
    //             "Addition mutation failed at offset {}: from 0x{:02X} expected 0x{:02X}, got 0x{:02X}",
    //             off, orig, expected, result
    //         );
    //         assert_eq!(
    //             buff.len(),
    //             len,
    //             "Buffer length changed after Addition mutation"
    //         );
    //     }
    // }

    // /// 测试 MutationDefinite::LengthFieldU16 随机 500 次
    // /// 操作：读取 u16 值（低端序），若该值 <= buff.len() 则置零，否则按位取反
    // #[test]
    // fn test_mutation_definite_length_field_random() {
    //     let dist = Distributions::new(vec!());
    //     let mut mutator = PrimitiveMutatorDefenite::new();
    //     let mut data = vec![0u8; 1024];
    //     // 保证有效数据至少有2字节
    //     let mut buff = DataBuff::new(&mut data, 0);

    //     for _ in 0..500 {
    //         let len = dist.gen_range(2, 1024);
    //         buff.set_to_random(len, &dist);
    //         // 随机选择一个 offset，确保 offset 和 offset+1 均合法
    //         let off = dist.gen_range(0, len - 1);

    //         let v_le = buff.read_u16(off, false);
    //         let v_be = buff.read_u16(off, true);
    //         let expected: u16 = if v_le <= (len as u16) || v_be <= (len as u16) {
    //             0
    //         } else {
    //             !v_le
    //         };

    //         let mutation = mutator.gen_length_field_mutation_u16_at_offset(&buff, off);
    //         let definite = MutationDefinite::LengthFieldU16(mutation);
    //         definite.apply(&mut buff);

    //         let result = buff.read_u16(off, false);
    //         assert_eq!(
    //             result, expected,
    //             "LengthField mutation failed at offset {}: expected 0x{:04X}, got 0x{:04X}",
    //             off, expected, result
    //         );
    //         assert_eq!(
    //             buff.len(),
    //             len,
    //             "Buffer length changed after LengthField mutation"
    //         );
    //     }
    // }

    // /// 测试 MutationDefinite::ChecksumField 随机 500 次
    // /// 操作：读取 offset 处及 offset+1 处的字节；
    // /// 若两字节差值 > 32，则对第二个字节全比特取反，否则仅翻转最低位
    // #[test]
    // fn test_mutation_definite_checksum_field_random() {
    //     let dist = Distributions::new(vec!());
    //     let mut mutator = PrimitiveMutatorDefenite::new();
    //     let mut data = vec![0u8; 1024];
    //     // 保证有效数据至少有2字节
    //     let mut buff = DataBuff::new(&mut data, 0);

    //     for _ in 0..500 {
    //         let len = dist.gen_range(2, 1024);
    //         buff.set_to_random(len, &dist);
    //         // 随机选择 offset，确保 offset+1 合法
    //         let off = dist.gen_range(0, len - 1);

    //         let a = buff.read_u8(off);
    //         let b = buff.read_u8(off + 1);
    //         let diff = if a > b { a - b } else { b - a };
    //         let threshold = 32;
    //         let expected = if diff > threshold {
    //             b ^ 0xff
    //         } else {
    //             b ^ 0x01
    //         };

    //         let mutation = mutator.gen_checksum_field_mutation_at_offset(&buff, off);
    //         let definite = MutationDefinite::ChecksumField(mutation);
    //         definite.apply(&mut buff);

    //         let result = buff.read_u8(off + 1);
    //         assert_eq!(
    //             result, expected,
    //             "ChecksumField mutation failed at offset {}: expected 0x{:02X}, got 0x{:02X}",
    //             off + 1, expected, result
    //         );
    //         assert_eq!(
    //             buff.len(),
    //             len,
    //             "Buffer length changed after ChecksumField mutation"
    //         );
    //     }
    // }

}
