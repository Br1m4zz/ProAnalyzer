use xxhash_rust::xxh3::xxh3_64;
// const  HASH_CONST:u32 = 0xa5b35705;

/// AFL++:  we switch from afl's murmur implementation to xxh3 as it is 30% faster -
///   and get 64 bit hashes instead of just 32 bit. Less collisions! :-)
//// we switch from afl's murmur implementation to xxh3 as it is 30% faster -
///and get 64 bit hashes instead of just 32 bit. Less collisions! :-) */
// pub fn hash32(key: &[u8], len: usize) -> u32 {
//     xxh3_64(&key[..len]) as u32
// }

/// AFL++:  we switch from afl's murmur implementation to xxh3 as it is 30% faster -
///   and get 64 bit hashes instead of just 32 bit. Less collisions! :-)
//// we switch from afl's murmur implementation to xxh3 as it is 30% faster -
///and get 64 bit hashes instead of just 32 bit. Less collisions! :-) */
pub fn hash64(key: &[u8], len: usize) -> u64 {
    xxh3_64(&key[..len])
}
