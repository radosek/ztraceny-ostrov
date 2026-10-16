//! String lists (Data.dat 32/33/34, reader 0x44e428): `u32 count; count × {u16 len; u8[len]}`,
//! each string XOR-chained with the previous cipher byte starting from 0x55, CP1250.

pub fn string_list(d: &[u8]) -> Option<Vec<String>> {
    let n = u32::from_le_bytes(d.get(..4)?.try_into().ok()?);
    let mut p = 4;
    let mut out = Vec::with_capacity(n as usize);
    for _ in 0..n {
        let len = u16::from_le_bytes(d.get(p..p + 2)?.try_into().ok()?) as usize;
        let c = d.get(p + 2..p + 2 + len)?;
        p += 2 + len;
        let mut k = 0x55;
        let plain: Vec<u8> = c
            .iter()
            .map(|&x| {
                let v = x ^ k;
                k = x;
                v
            })
            .collect();
        out.push(encoding_rs::WINDOWS_1250.decode_without_bom_handling(&plain).0.into_owned());
    }
    Some(out)
}
