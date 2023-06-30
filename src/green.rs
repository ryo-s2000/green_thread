use nix::sys::mman::{mprotect, ProtFlags};
use rand;
use std::alloc::{alloc, dealloc, Layout};
use std::collections::{HashMap, HashSet, LinkedList};
use std::ffi::c_void;
use std::ptr;

static mut CTX_MAIN: Option<Box<Registers>> = None;
static mut UNUSED_STACK: (*mut u8, Layout) = (ptr::null_mut(), Layout::new::<u8>());
static mut CONTEXTS: LinkedList<Box<Context>> = LinkedList::new();
static mut ID: *mut HashSet<u64> = ptr::null_mut();

#[repr(C)]
struct Registers {
    // callee save resisters
    d8: u64, d9: u64, d10: u64, d11: u64, d12: u64, d13: u64, d14: u64, d15: u64,
    x19: u64, x20: u64, x21: u64, x22: u64, x23: u64, x24: u64, x25: u64, x26: u64, x27: u64, x28: u64,

    x30: u64, // link resister
    sp: u64, // stack pointer
}

impl Registers {
    fn new (sp: u64) -> Self {
        Registers {
            d8: 0, d9: 0, d10: 0, d11: 0, d12: 0, d13: 0, d14: 0, d15: 0,
            x19: 0, x20: 0, x21: 0, x22: 0, x23: 0, x24: 0, x25: 0, x26: 0, x27: 0, x28: 0,
            x30: entry_point as u64,
            sp,
        }
    }
}

extern "C" {
    fn set_context(ctx: *mut Registers) -> u64;
    fn switch_context(ctx: *const Registers) -> !;
}

extern "C" fn entry_point() {
    unsafe {
        let ctx = CONTEXTS.front().unwrap();
        ((**ctx).entry)();
        let ctx = CONTEXTS.pop_front().unwrap();
        (*ID).remove(&ctx.id);

        UNUSED_STACK = ((*ctx).stack, (*ctx).stack_layout);

        match CONTEXTS.front() {
            Some(c) => {
                switch_context((**c).get_regs());
            }
            None => {
                if let Some(c) = &CTX_MAIN {
                    switch_context(&**c as *const Registers);
                }
            }
        };
    }
    panic!("entry_point");
}

pub fn spawn_from_main(func: Entry, stack_size: usize) {
    unsafe {
        if let Some(_) = &CTX_MAIN {
            panic!("spawm_main is called twice");
        }

        CTX_MAIN = Some(Box::new(Registers::new(0)));
        if let Some(ctx) = &mut CTX_MAIN {
            let mut msgs = LinkedList::new();
            let mut MESSAGES = &mut msgs as *mut LinkedList<u64>;

            let mut waiting = HashMap::new();
            let mut WAITING = &mut waiting as *mut HashMap<u64, Box<Context>>;

            let mut ids = HashSet::new();
            ID = &mut ids as *mut HashSet<u64>;

            if set_context(&mut **ctx as *mut Registers) == 0 {
                CONTEXTS.push_back(Box::new(Context::new(func, stack_size, get_id())));
                let first = CONTEXTS.front().unwrap();
                switch_context(first.get_regs());
            }

            rm_unused_stack();

            CTX_MAIN = None;
            CONTEXTS.clear();
            MESSAGES = ptr::null_mut();
            WAITING = ptr::null_mut();
            ID = ptr::null_mut();

            msgs.clear();
            waiting.clear();
            ids.clear();
        }
    }
}

unsafe fn rm_unused_stack() {
    if UNUSED_STACK.0 != ptr::null_mut() {
        mprotect(
            UNUSED_STACK.0 as *mut c_void,
            PAGE_SIZE,
            ProtFlags::PROT_READ | ProtFlags::PROT_WRITE,
        ).unwrap();
        dealloc(UNUSED_STACK.0, UNUSED_STACK.1);
        UNUSED_STACK = (ptr::null_mut(), Layout::new::<u8>());
    }
}

type Entry = fn();

const PAGE_SIZE: usize = 4 * 1024;

struct Context {
    regs: Registers,
    stack: *mut u8,
    stack_layout: Layout,
    entry: Entry,
    id: u64,
}

impl Context {
    fn get_regs_mut(&mut self) -> *mut Registers {
        &mut self.regs as *mut Registers
    }

    fn get_regs(&self) -> *const Registers {
        &self.regs as *const Registers
    }

    fn new(func: Entry, stack_size: usize, id: u64) -> Self {
        let layout = Layout::from_size_align(stack_size, PAGE_SIZE).unwrap();
        let stack = unsafe { alloc(layout) };

        unsafe {
            mprotect(stack as *mut c_void, PAGE_SIZE, ProtFlags::PROT_NONE).unwrap();
        }

        let regs = Registers::new(stack as u64 + stack_size as u64);

        Context {
            regs: regs,
            stack: stack,
            stack_layout: layout,
            entry: func,
            id: id,
        }
    }
}

fn get_id() -> u64 {
    loop {
        let rnd = rand::random::<u64>();
        unsafe {
            if !(*ID).contains(&rnd) {
                (*ID).insert(rnd);
                return rnd;
            }
        }
    }
}

pub fn spawn(func: Entry, stack_size: usize) -> u64 {
    unsafe {
        let id = get_id();
        CONTEXTS.push_back(Box::new(Context::new(func, stack_size, id)));
        schedule();
        id
    }
}

pub fn schedule() {
    unsafe {
        if CONTEXTS.len() == 1 {
            return;
        }

        let mut ctx = CONTEXTS.pop_front().unwrap();
        let regs = ctx.get_regs_mut();
        CONTEXTS.push_back(ctx);

        if set_context(regs) == 0 {
            let next = CONTEXTS.front().unwrap();
            switch_context((**next).get_regs());
        }

        rm_unused_stack();
    }
}
