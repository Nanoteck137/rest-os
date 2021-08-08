# Memory Map

[0x0000000000000000-0x00007fffffffffff]  128 TiB User Space
[0xffff800000000000-0xffffffffffffffff]  128 TiB Kernel Space

Kernel Space:
[0xffff888000000000-0xffff987fffffffff]   16 TiB Physical Memory Mapping
[0xffff988000000000-0xffffa87fffffffff]   16 TiB Empty Hole
[0xffffa88000000000-0xffffb87fffffffff]   16 TiB Vmalloc
[0xffffffff80000000-0xffffffffbfffffff]    1 GiB Kernel Text

# Memory Manager

* Global Locked
* Shared between cpus

```rust
let page_table = ;

let region = create region where kernel virtual memory should be;

let allocator = Allocator::new(region);

fn allocate_kernel_vm() -> Region {
    allocator.allocate_memory()
}

// Allocates contiguous virtual memory (don't care about the backing store)
let region = mm::allocate_kernel_vm();
let vaddr = region.vaddr;
```
