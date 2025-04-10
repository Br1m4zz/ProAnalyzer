use rand::distributions::weighted::alias_method::WeightedIndex;
use rand::prelude::*;

pub struct Choices<T> {
    weights: WeightedIndex<usize>,
    options: Vec<T>,
}

impl<T> Choices<T> {
    /// 创建一个可以根据权重weights随机选择options中元素的结构体
    pub fn new(weights: Vec<usize>, options: Vec<T>) -> Self {
        let weights = WeightedIndex::new(weights).unwrap();
        return Self { weights, options };
    }

    ///根据每个选项options的权重，随机地从一组选项options中选择一个
    pub fn sample<'a, R: Rng>(&'a self, rng: &mut R) -> &'a T {
        let i = self.weights.sample(rng);
        return &self.options[i];
    }
}
