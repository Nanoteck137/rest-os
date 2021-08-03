def get_addr_index(addr):
    p1 = (addr >> 12) & 0x1ff
    p2 = (addr >> 21) & 0x1ff
    p3 = (addr >> 30) & 0x1ff
    p4 = (addr >> 39) & 0x1ff
    return (p4, p3, p2, p1)
