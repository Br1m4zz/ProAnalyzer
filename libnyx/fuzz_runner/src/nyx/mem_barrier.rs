use std::sync::atomic::compiler_fence;
use std::sync::atomic::Ordering;
//如何使用Rust的std::sync::atomic::compiler_fence函数来防止编译器优化掉连续的内存读取操作。
//这通常用于确保对共享内存缓冲区的读/写操作在多线程环境中按预期执行，而不会因为编译器优化而被省略。
// we expect this to be a nop.
// but in some extreme cases, this
/*
use std::sync::atomic::compiler_fence;
use std::sync::atomic::Ordering;

fn barrier() {
compiler_fence(Ordering::SeqCst);
}
// 示例函数，演示如何使用内存屏障
pub fn read2(data: &mut u32) -> u32{
    let a = *data;
    barrier();
    let b = *data;
    return a.wrapping_add(b);
}
*/

//compiles to
/*
        mov     eax, dword ptr [rdi]
        add     eax, dword ptr [rdi]
        ret
*/
//while the second access gets optimized out without the barrier.
//To ensure that reads/writes to the shared memory buffer actually are executed, we use mem_barrier to lightweight synchronize the values.
// 定义一个内存屏障函数
pub fn mem_barrier() {
    compiler_fence(Ordering::SeqCst);
}
