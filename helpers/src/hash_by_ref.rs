use std::rc::Rc;
use std::ptr;
use std::hash::{Hash,Hasher};
//这段Rust代码定义了一个泛型结构体：HashAsRef<T>。
//它包装了一个引用计数智能指针Rc<T>。这个结构体实现了几个trait（接口），让它可以在哈希集合（如HashSet或HashMap）中使用
#[derive(Clone,Debug)]
pub struct HashAsRef<T>(Rc<T>);

impl<T> Hash for HashAsRef<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let ptr: *const T = &*self.0;
        ptr::hash(ptr, state);
    }
}

impl<T> PartialEq for HashAsRef<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<T> Eq for HashAsRef<T> {}

impl<T> HashAsRef<T>{
    pub fn new(rc: Rc<T>) -> Self{
        return Self(rc);
    }
    pub fn into_rc(self) -> Rc<T>{
        self.0
    }
    pub fn as_usize(&self) -> usize{
        let ptr: *const T = &*self.0;
        return ptr as usize;
    }
}