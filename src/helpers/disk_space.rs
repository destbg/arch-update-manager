use std::ffi::CString;

pub fn available_bytes(path: &str) -> Option<u64> {
    let c_path = CString::new(path).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };

    let result = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if result != 0 {
        return None;
    }

    let block_size = if stat.f_frsize != 0 {
        stat.f_frsize
    } else {
        stat.f_bsize
    };

    return (stat.f_bavail as u64).checked_mul(block_size as u64);
}
