//! Reference capture for the original Ostrov.exe under Wine, loaded as `ddraw.dll` from the
//! game folder (it forwards DirectDraw to the system DLL).
//!
//! - `timeGetTime`/`GetTickCount` return a virtual clock that advances 1 ms per call, so every
//!   busy-wait loop ends after the same number of iterations on every run.
//! - `GetSystemTime` returns 2000-01-01 00:00:00.000, so `Randomize` seeds RandSeed with 0.
//! - `GetCursorPos` returns the scripted pointer.
//! - The back buffer is copied when the game unlocks it (0x44ee44) and hashed when it flips
//!   (0x44d380); scripted input for frame N is posted as window messages right after flip N.
//!
//! Script `probe.txt` (game folder), one command per line: `<frame> move|down|rdown <x> <y>`,
//! `<frame> up`, `<frame> key <vk>`, `<frame> char <byte>`, `<frame> dump`, `<frame> quit`.
//! Output: `probe.log` lines `<frame> <vtime> <fnv64>`, and `frame_<N>.raw` (640×480 u16 LE).

#![allow(clippy::missing_safety_doc)]

use std::{
    ffi::c_void,
    fs::File,
    io::{BufRead, BufReader, Write},
    sync::{
        Mutex,
        atomic::{AtomicI32, AtomicU32, Ordering::Relaxed},
    },
};
use windows_sys::Win32::{
    Foundation::{HWND, LPARAM, POINT, SYSTEMTIME, WPARAM},
    System::{
        LibraryLoader::{GetModuleHandleA, GetProcAddress, LoadLibraryA},
        Memory::{PAGE_EXECUTE_READWRITE, VirtualProtect},
        Threading::ExitProcess,
    },
    UI::WindowsAndMessaging::{
        FindWindowA, FindWindowExA, PostMessageA, WM_CHAR, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
        WM_RBUTTONDOWN, WM_RBUTTONUP,
    },
};

const W: usize = 640;
const H: usize = 480;

static VTIME: AtomicU32 = AtomicU32::new(0);
static CX: AtomicI32 = AtomicI32::new(320);
static CY: AtomicI32 = AtomicI32::new(240);
static mut LOCKED_FRAME: [u16; W * H] = [0; W * H];
static FLIPS: AtomicU32 = AtomicU32::new(0);

struct State {
    script: Vec<(u32, String, Vec<i32>)>,
    log: Option<File>,
}

static STATE: Mutex<State> = Mutex::new(State { script: Vec::new(), log: None });

fn game_dir_file(name: &str) -> String {
    format!("C:\\ostrov\\{name}")
}

fn init_script() {
    let mut st = STATE.lock().unwrap();
    if let Ok(f) = File::open(game_dir_file("probe.txt")) {
        for line in BufReader::new(f).lines().map_while(Result::ok) {
            let mut it = line.split_whitespace();
            let (Some(n), Some(cmd)) = (it.next().and_then(|n| n.parse().ok()), it.next()) else { continue };
            st.script.push((n, cmd.to_string(), it.filter_map(|a| a.parse().ok()).collect()));
        }
    }
    st.log = File::create(game_dir_file("probe.log")).ok();
}

// ---- imported API replacements ---------------------------------------------------------

static CALLS: AtomicU32 = AtomicU32::new(0);

unsafe extern "system" fn time_get_time() -> u32 {
    if CALLS.fetch_add(1, Relaxed) % 100_000 == 0 {
        if let Ok(mut st) = STATE.try_lock() {
            if let Some(l) = st.log.as_mut() {
                let _ = writeln!(l, "# vtime {}", VTIME.load(Relaxed));
            }
        }
    }
    VTIME.fetch_add(1, Relaxed).wrapping_add(1)
}

unsafe extern "system" fn get_system_time(t: *mut SYSTEMTIME) {
    unsafe {
        *t = SYSTEMTIME { wYear: 2000, wMonth: 1, wDayOfWeek: 6, wDay: 1, wHour: 0, wMinute: 0, wSecond: 0, wMilliseconds: 0 };
    }
}

unsafe extern "system" fn get_cursor_pos(p: *mut POINT) -> i32 {
    unsafe {
        (*p).x = CX.load(Relaxed);
        (*p).y = CY.load(Relaxed);
    }
    1
}

/// Point every import slot of Ostrov.exe that holds `dll!name` at `f`. Delphi leaves
/// OriginalFirstThunk empty, so slots are matched by their resolved address.
unsafe fn patch_iat(dll: &core::ffi::CStr, name: &core::ffi::CStr, f: usize) {
    unsafe {
        let Some(real) = GetProcAddress(LoadLibraryA(dll.as_ptr() as _), name.as_ptr() as _) else { return };
        let real = real as usize;
        let base = GetModuleHandleA(std::ptr::null()) as usize;
        let rd = |o: usize| *((base + o) as *const u32) as usize;
        let pe = rd(0x3c);
        let mut d = rd(pe + 0x80);
        while rd(d + 12) != 0 {
            let mut slot = base + rd(d + 16);
            while *(slot as *const usize) != 0 {
                if *(slot as *const usize) == real {
                    let mut old = 0;
                    VirtualProtect(slot as _, 4, PAGE_EXECUTE_READWRITE, &mut old);
                    *(slot as *mut usize) = f;
                }
                slot += 4;
            }
            d += 20;
        }
    }
}

// ---- inline hooks in Ostrov.exe -------------------------------------------------------------

/// Called at the entry of the surface Unlock wrapper 0x44ee44 while the back buffer is locked.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn probe_unlock() {
    unsafe {
        if *(0x468b48 as *const u8) == 0 {
            return;
        }
        let ptr = *(0x46ce98 as *const usize) as *const u8;
        let pitch = *(0x46ce9c as *const i32) as usize;
        if ptr.is_null() {
            return;
        }
        for y in 0..H {
            let row = ptr.add(y * pitch) as *const u16;
            let dst = &raw mut LOCKED_FRAME;
            std::ptr::copy_nonoverlapping(row, (*dst).as_mut_ptr().add(y * W), W);
        }
    }
}

fn fnv(px: &[u16]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &p in px {
        for b in p.to_le_bytes() {
            h ^= b as u64;
            h = h.wrapping_mul(0x100_0000_01b3);
        }
    }
    h
}

unsafe fn target() -> HWND {
    unsafe {
        let form = FindWindowA(c"TMainForm".as_ptr() as _, std::ptr::null());
        let dx = FindWindowExA(form, std::ptr::null_mut(), c"TDXDraw".as_ptr() as _, std::ptr::null());
        if dx.is_null() { form } else { dx }
    }
}

/// Called at the entry of DXDraw.Flip 0x44d380.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn probe_flip() {
    unsafe {
        let n = FLIPS.fetch_add(1, Relaxed) + 1;
        let frame = &*(&raw const LOCKED_FRAME);
        let mut st = STATE.lock().unwrap();
        let h = fnv(frame);
        if let Some(l) = st.log.as_mut() {
            let _ = writeln!(l, "{n} {} {h:016x}", VTIME.load(Relaxed));
        }
        let due: Vec<_> = st.script.iter().filter(|s| s.0 == n).cloned().collect();
        drop(st);
        let hwnd = target();
        let lp = |x: i32, y: i32| ((y as u32) << 16 | (x as u32 & 0xffff)) as LPARAM;
        for (_, cmd, a) in due {
            let (x, y) = (a.first().copied().unwrap_or(CX.load(Relaxed)), a.get(1).copied().unwrap_or(CY.load(Relaxed)));
            match cmd.as_str() {
                "move" => {
                    CX.store(x, Relaxed);
                    CY.store(y, Relaxed);
                    PostMessageA(hwnd, WM_MOUSEMOVE, 0, lp(x, y));
                }
                "down" => {
                    CX.store(x, Relaxed);
                    CY.store(y, Relaxed);
                    PostMessageA(hwnd, WM_MOUSEMOVE, 0, lp(x, y));
                    PostMessageA(hwnd, WM_LBUTTONDOWN, 1, lp(x, y));
                }
                "rdown" => {
                    CX.store(x, Relaxed);
                    CY.store(y, Relaxed);
                    PostMessageA(hwnd, WM_MOUSEMOVE, 0, lp(x, y));
                    PostMessageA(hwnd, WM_RBUTTONDOWN, 2, lp(x, y));
                }
                "up" => {
                    PostMessageA(hwnd, WM_LBUTTONUP, 0, lp(CX.load(Relaxed), CY.load(Relaxed)));
                }
                "rup" => {
                    PostMessageA(hwnd, WM_RBUTTONUP, 0, lp(CX.load(Relaxed), CY.load(Relaxed)));
                }
                "key" => {
                    PostMessageA(hwnd, WM_KEYDOWN, x as WPARAM, 1);
                }
                "char" => {
                    PostMessageA(hwnd, WM_CHAR, x as WPARAM, 1);
                }
                "dump" => {
                    if let Ok(mut f) = File::create(game_dir_file(&format!("frame_{n}.raw"))) {
                        let bytes: Vec<u8> = frame.iter().flat_map(|p| p.to_le_bytes()).collect();
                        let _ = f.write_all(&bytes);
                    }
                }
                "quit" => ExitProcess(0),
                _ => {}
            }
        }
    }
}

// Unlock 0x44ee44: `cmp byte ptr [0x468b48], 0` (7 bytes) is replaced by a jump here.
std::arch::global_asm!(
    ".globl _probe_unlock_stub",
    "_probe_unlock_stub:",
    "pushad",
    "call _probe_unlock",
    "popad",
    "cmp byte ptr [0x468b48], 0",
    "push 0x44ee4b",
    "ret",
    // Flip 0x44d380: `push ebx; mov ebx, eax; cmp byte ptr [ebx+0x204], 0` (10 bytes).
    ".globl _probe_flip_stub",
    "_probe_flip_stub:",
    "pushad",
    "call _probe_flip",
    "popad",
    "push ebx",
    "mov ebx, eax",
    "cmp byte ptr [ebx + 0x204], 0",
    "push 0x44d38a",
    "ret",
);

unsafe extern "C" {
    fn probe_unlock_stub();
    fn probe_flip_stub();
}

/// `jmp rel32` at `at` to `to`, padding the rest of `len` bytes with nops. Flags are not live
/// across the replaced compare because the stub re-executes it last.
unsafe fn jump(at: usize, to: usize, len: usize) {
    unsafe {
        let mut old = 0;
        VirtualProtect(at as _, len, PAGE_EXECUTE_READWRITE, &mut old);
        let p = at as *mut u8;
        *p = 0xe9;
        *(p.add(1) as *mut i32) = (to as i32).wrapping_sub(at as i32 + 5);
        for i in 5..len {
            *p.add(i) = 0x90;
        }
    }
}

/// `0 autoskip`: while no frame is flipped for 1.5 s of real time (videos play through MCI,
/// not DirectDraw), post ESC so the video ends.
fn autoskip() {
    std::thread::spawn(|| {
        if let Some(l) = STATE.lock().unwrap().log.as_mut() {
            let _ = writeln!(l, "# autoskip thread running");
        }
        let mut last = u32::MAX;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(1500));
            let n = FLIPS.load(Relaxed);
            if n == last {
                unsafe {
                    let hwnd = target();
                    PostMessageA(hwnd, WM_KEYDOWN, 0x1b, 1);
                }
                if let Some(l) = STATE.lock().unwrap().log.as_mut() {
                    let _ = writeln!(l, "# autoskip after frame {n}");
                }
            }
            last = n;
        }
    });
}

unsafe fn install() {
    unsafe {
        init_script();
        let (skip, novideo) = {
            let st = STATE.lock().unwrap();
            (st.script.iter().any(|s| s.1 == "autoskip"), st.script.iter().any(|s| s.1 == "novideo"))
        };
        if skip {
            autoskip();
        }
        if novideo {
            // 0x464c0c PlayVideo(form, name, fx): register convention, no stack args.
            let mut old = 0;
            VirtualProtect(0x464c0c as _, 1, PAGE_EXECUTE_READWRITE, &mut old);
            *(0x464c0c as *mut u8) = 0xc3;
        }
        patch_iat(c"winmm.dll", c"timeGetTime", time_get_time as *const () as usize);
        patch_iat(c"kernel32.dll", c"GetTickCount", time_get_time as *const () as usize);
        patch_iat(c"kernel32.dll", c"GetSystemTime", get_system_time as *const () as usize);
        patch_iat(c"user32.dll", c"GetCursorPos", get_cursor_pos as *const () as usize);
        jump(0x44ee44, probe_unlock_stub as *const () as usize, 7);
        jump(0x44d380, probe_flip_stub as *const () as usize, 10);
        if let Some(l) = STATE.lock().unwrap().log.as_mut() {
            let _ = writeln!(l, "# hooks installed");
        }
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn DllMain(_: *mut c_void, reason: u32, _: *mut c_void) -> i32 {
    if reason == 1 {
        unsafe { install() };
    }
    1
}

// ---- DirectDraw forwarding ---------------------------------------------------------------

unsafe fn real(name: &core::ffi::CStr) -> usize {
    unsafe {
        let h = LoadLibraryA(c"C:\\ostrov\\ddraw_aqrit.dll".as_ptr() as _);
        GetProcAddress(h, name.as_ptr() as _).map_or(0, |f| f as usize)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DirectDrawCreate(a: *mut c_void, b: *mut c_void, c: *mut c_void) -> i32 {
    unsafe {
        let f: extern "system" fn(*mut c_void, *mut c_void, *mut c_void) -> i32 = std::mem::transmute(real(c"DirectDrawCreate"));
        f(a, b, c)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DirectDrawCreateEx(a: *mut c_void, b: *mut c_void, c: *mut c_void, d: *mut c_void) -> i32 {
    unsafe {
        let f: extern "system" fn(*mut c_void, *mut c_void, *mut c_void, *mut c_void) -> i32 =
            std::mem::transmute(real(c"DirectDrawCreateEx"));
        f(a, b, c, d)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DirectDrawEnumerateA(a: *mut c_void, b: *mut c_void) -> i32 {
    unsafe {
        let f: extern "system" fn(*mut c_void, *mut c_void) -> i32 = std::mem::transmute(real(c"DirectDrawEnumerateA"));
        f(a, b)
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "system" fn DirectDrawEnumerateExA(a: *mut c_void, b: *mut c_void, c: u32) -> i32 {
    unsafe {
        let f: extern "system" fn(*mut c_void, *mut c_void, u32) -> i32 = std::mem::transmute(real(c"DirectDrawEnumerateExA"));
        f(a, b, c)
    }
}

/// Homebrew's i686 mingw libgcc uses SJLJ unwinding; Rust's std expects the DWARF entry
/// point. Panics abort here, so it is never reached.
#[unsafe(no_mangle)]
pub extern "C" fn _Unwind_Resume() -> ! {
    unsafe { ExitProcess(3) }
}
